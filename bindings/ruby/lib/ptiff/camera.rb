# frozen_string_literal: true

module PTiff
  # Camera is a structured camera calibration for one frame.
  #
  # It wraps the C-ABI `Ptiff::ptiff_open_path_camera` result (K, [R|t], P and
  # the ISO-8601 timestamp) as a small value object. Matrices are returned as
  # flat Arrays of Floats (row-major); the projection P = K * [R|t] is computed
  # in the C++ library.
  #
  # A Camera can be built two ways:
  #   * `Camera.from_path(path)` -- read the calibration embedded in a PTIFF.
  #   * `Camera.new(focal_length_x: ..., focal_length_y: ..., principal_x: ...,
  #                  principal_y: ..., rotation_w: ..., ...)` -- describe a
  #     camera from known parameters, then persist it when creating an image via
  #     `Image.create(..., camera: cam)`.
  class Camera
    attr_reader :data

    def initialize(data = nil,
      focal_length_x: 0.0,
      focal_length_y: 0.0,
      principal_x: 0.0,
      principal_y: 0.0,
      rotation_w: 1.0,
      rotation_x: 0.0,
      rotation_y: 0.0,
      rotation_z: 0.0,
      position_x: 0.0,
      position_y: 0.0,
      position_z: 0.0,
      model: "pinhole",
      timestamp: "")
      @data = data ? data.dup : {
        "has_intrinsics" => 1,
        "has_extrinsics" => 1,
        "focal_length_x" => focal_length_x.to_f,
        "focal_length_y" => focal_length_y.to_f,
        "principal_x" => principal_x.to_f,
        "principal_y" => principal_y.to_f,
        "rotation_w" => rotation_w.to_f,
        "rotation_x" => rotation_x.to_f,
        "rotation_y" => rotation_y.to_f,
        "rotation_z" => rotation_z.to_f,
        "position_x" => position_x.to_f,
        "position_y" => position_y.to_f,
        "position_z" => position_z.to_f,
        "model" => model,
        "timestamp" => timestamp,
      }
    end

    # Opens the TIFF/BigTIFF file at path and returns its structured camera
    # calibration. Raises IOError on C-ABI failure.
    def self.from_path(path)
      rc, data = Ptiff::ptiff_open_path_camera(path.to_s)
      raise IOError, "ptiff_open_path_camera failed for #{path.inspect} (rc=#{rc})" unless rc.zero?
      new(data)
    end

    def has_intrinsics = @data["has_intrinsics"] != 0
    def has_extrinsics = @data["has_extrinsics"] != 0
    def model          = @data["model"] || "pinhole"
    def focal_length_x = @data["focal_length_x"]
    def focal_length_y = @data["focal_length_y"]
    def principal_x    = @data["principal_x"]
    def principal_y    = @data["principal_y"]
    def rotation_w     = @data["rotation_w"]
    def rotation_x     = @data["rotation_x"]
    def rotation_y     = @data["rotation_y"]
    def rotation_z     = @data["rotation_z"]
    def position_x     = @data["position_x"]
    def position_y     = @data["position_y"]
    def position_z     = @data["position_z"]
    def timestamp      = @data["timestamp"]

    def intrinsics_matrix = Array(@data["intrinsics"])
    def extrinsics_matrix = Array(@data["extrinsics"])
    def projection_matrix = Array(@data["projection"])

    # Builds a SWIG `Ptiff::Ptiff_camera` struct ready to persist with
    # `Image.create(..., camera: cam)`.
    def to_struct
      c = Ptiff::Ptiff_camera.new
      c.has_intrinsics = has_intrinsics ? 1 : 0
      c.focal_length_x = focal_length_x.to_f
      c.focal_length_y = focal_length_y.to_f
      c.principal_x = principal_x.to_f
      c.principal_y = principal_y.to_f
      c.has_extrinsics = has_extrinsics ? 1 : 0
      c.rotation_w = rotation_w.to_f
      c.rotation_x = rotation_x.to_f
      c.rotation_y = rotation_y.to_f
      c.rotation_z = rotation_z.to_f
      c.position_x = position_x.to_f
      c.position_y = position_y.to_f
      c.position_z = position_z.to_f
      c.timestamp = timestamp.to_s
      c
    end

    def inspect
      "#<PTiff::Camera fx=#{focal_length_x} fy=#{focal_length_y} " \
        "cx=#{principal_x} cy=#{principal_y} model=#{model}>"
    end
  end
end
