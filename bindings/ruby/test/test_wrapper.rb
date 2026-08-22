# frozen_string_literal: true

# Tests for the idiomatic PTiff object layer (Camera/Image/Tile/Logger/Metadata),
# which wraps the raw SWIG low-level surface. Mirrors the Python src/ptiff/
# wrapper tests.
require 'minitest/autorun'
require 'tmpdir'
require 'ptiff'

FIXTURE = File.expand_path('../../../scripts/samples/ptiff_interop_fixture.tif', __dir__)

class TestPTiffMetadata < Minitest::Test
  def test_reads_descriptor_and_fields
    m = PTiff::Metadata.new(FIXTURE)
    assert_equal 128, m.width
    assert_equal 128, m.height
    assert_equal 1, m.channel_count
    assert_equal 'UInt8', m.pixel_type_name
    assert_equal 128, m.tile_width
    assert_equal 128, m.tile_height
    assert_equal 'pinhole', m.field('ptiff.camera.model')
    assert_equal 'IAU_MOON', m.field('ptiff.spice.frame')
    refute_empty m.fields
  end
end

class TestPTiffCamera < Minitest::Test
  def test_from_path
    cam = PTiff::Camera.from_path(FIXTURE)
    assert cam.has_intrinsics
    assert_in_delta 700.0, cam.focal_length_x
    assert_in_delta 700.0, cam.focal_length_y
    assert_in_delta 64.0, cam.principal_x
    assert_in_delta 64.0, cam.principal_y
    refute cam.has_extrinsics
    assert_equal 9, cam.intrinsics_matrix.length
    assert_equal 12, cam.extrinsics_matrix.length
    assert_equal 12, cam.projection_matrix.length
  end

  def test_build_and_to_struct
    cam = PTiff::Camera.new(focal_length_x: 700.0, principal_x: 64.0)
    assert cam.has_intrinsics
    assert_equal 'pinhole', cam.model
    struct = cam.to_struct
    assert_in_delta 700.0, struct.focal_length_x
    assert_in_delta 64.0, struct.principal_x
  end
end

class TestPTiffLogger < Minitest::Test
  def test_level_roundtrip
    log = PTiff::Logger.new
    original = log.level
    begin
      log.level = PTiff::Logger::ERROR
      assert_equal PTiff::Logger::ERROR, log.level
      log.debug('filtered out')
      log.error('emitted')
    ensure
      log.level = original
    end
  end
end

class TestPTiffImage < Minitest::Test
  def test_create_write_read_roundtrip
    Dir.mktmpdir do |dir|
      out = File.join(dir, 'rt.tif')
      img = PTiff::Image.create(out, 32, 32, pixel_type: 0, tile_width: 16, tile_height: 16)
      assert_equal 32, img.width
      assert_equal 2, img.tile_columns
      assert_equal 2, img.tile_rows
      bs = img.tile_byte_size
      assert_equal 256, bs
      img.write_tile(0, 0, ("\x07".b) * bs)
      img.write_tile(1, 1, ("\x09".b) * bs)
      img.close

      rd = PTiff::Image.open(out)
      assert_equal 32, rd.width
      assert_equal 32, rd.height
      assert_equal 7, rd.read_tile(0, 0).data.getbyte(0)
      assert_equal 9, rd.read_tile(1, 1).data.getbyte(0)
      rd.close
    end
  end

  def test_create_with_camera_roundtrip
    Dir.mktmpdir do |dir|
      out = File.join(dir, 'cam.tif')
      cam = PTiff::Camera.new(focal_length_x: 700.0, focal_length_y: 715.0,
                              principal_x: 32.0, principal_y: 24.0,
                              rotation_w: 1.0, position_x: 1.0, position_y: 2.0,
                              position_z: 3.0, timestamp: '2026-08-21T12:34:56.000Z')
      img = PTiff::Image.create(out, 32, 32, pixel_type: 0, tile_width: 16,
                                tile_height: 16, camera: cam)
      img.write_tile(0, 0, ("\0".b) * img.tile_byte_size)
      img.close

      rd = PTiff::Image.open(out)
      got = rd.camera
      assert got.has_intrinsics
      assert_in_delta 700.0, got.focal_length_x
      assert_in_delta 715.0, got.focal_length_y
      assert_equal '2026-08-21T12:34:56.000Z', got.timestamp
      # Identity rotation + t=(1,2,3): P = K*[R|t] -> translation column.
      assert_in_delta 700 * 1 + 32 * 3, got.projection_matrix[3]
      assert_in_delta 715 * 2 + 24 * 3, got.projection_matrix[7]
      assert_in_delta 3.0, got.projection_matrix[11]
      rd.close
    end
  end
end
