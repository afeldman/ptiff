# Julia test suite for the Ptiff binding.
#
# Run from the repository root or from bindings/julia:
#
#     julia --project=bindings/julia bindings/julia/test/runtests.jl
#
# or, via the Makefile:
#
#     make -C bindings/julia test
#
# Requires that the C ABI library has been built (cargo build -p ptiff-c --release).

using Test
using Ptiff

# Absolute path to the shared C++-Oracle interop fixture that every binding
# validates against (ptiff.camera.model=pinhole, ptiff.spice.frame=IAU_MOON,
# intrinsics-only camera). Resolved from this test dir: bindings/julia/test -> ../../../../
# is repo root.
function fixture_path()
    here = @__DIR__
    return abspath(joinpath(here, "..", "..", "..", "scripts", "samples", "ptiff_interop_fixture.tif"))
end

@testset "Ptiff.jl" begin
    mktempdir() do dir
        out = joinpath(dir, "roundtrip_ptiff.tif")

        @testset "version" begin
            v = Ptiff.version()
            @test v isa Ptiff.Version
            c = Ptiff.compile_time_version()
            @test c == v                       # Rust core is statically linked
            @test v.major >= 1
        end

        @testset "backends" begin
            names = Ptiff.backends()
            @test "tiff" in names
            # `pds4` is a metadata/storage domain inside the TIFF backend, not a
            # separately registered decoder backend, so it is NOT asserted here.
        end

        @testset "logging" begin
            old = Ptiff.logger_level()
            Ptiff.logger_set_level(Ptiff.LogInfo)
            @test Ptiff.logger_level() == Int32(Ptiff.LogInfo)
            Ptiff.logger_log(Ptiff.LogDebug, "julia test")
            Ptiff.logger_set_level(Ptiff.LogLevel(old))
        end

        @testset "write / read round-trip" begin
            w = 32; h = 32
            tw = 16; th = 16
            sink = Ptiff.create_sink(out, w, h, Ptiff.PTIFF_PIXEL_UINT8, 1;
                                        tile_width=tw, tile_height=th,
                                        gsd=1.0)
            cols, rows, tbs = Ptiff.sink_info(sink)
            @test (cols, rows) == (2, 2)
            @test tbs == tw * th

            fill_val = UInt8(7)
            tile = fill(fill_val, tbs)
            for r in 0:(rows - 1), c in 0:(cols - 1)
                Ptiff.write_tile(sink, c, r, tile)
            end
            Ptiff.close_sink(sink)

            # info
            info = Ptiff.read_info(out)
            @test info.width == w
            @test info.height == h
            @test info.pixel_type == Ptiff.PTIFF_PIXEL_UINT8
            @test info.channel_count == 1
            @test info.tile_width == tw
            @test info.tile_height == th
            # GSD is format-neutral (dropped by a TIFF round-trip; see the
            # core scene_serializer), so a written GSD is NOT persisted.
            @test info.gsd === nothing

            # read-back
            src = Ptiff.open_source(out)
            si = Ptiff.source_info(src)
            @test si.width == w && si.height == h

            # tile indexing: read (0,0) -> all 7s
            data = Ptiff.read_tile(src, 0, 0)
            @test length(data) == tbs
            @test all(==(fill_val), data)

            # edge tile (1,1) is still padded to uniform size
            data22 = Ptiff.read_tile(src, 1, 1)
            @test length(data22) == tbs

            Ptiff.close_source(src)
        end

        @testset "metadata (empty on plain file)" begin
            meta = Ptiff.read_metadata(out)
            # plain round-trip file carries no user metadata fields
            @test meta isa Dict{String,String}
        end

        @testset "interop fixture (metadata + camera)" begin
            fx = fixture_path()
            if isfile(fx)
                meta = Ptiff.read_metadata(fx)
                @test meta isa Dict{String,String}
                @test !isempty(meta)
                @test meta["ptiff.camera.model"] == "pinhole"
                @test meta["ptiff.camera.focal_length_x"] == "700.0"
                @test meta["ptiff.camera.focal_length_y"] == "700.0"
                @test meta["ptiff.camera.principal_x"] == "64.000000"
                @test meta["ptiff.camera.principal_y"] == "64.000000"
                @test meta["ptiff.spice.frame"] == "IAU_MOON"

                cam = Ptiff.read_camera(fx)
                @test haskey(cam, "intrinsics")
                @test haskey(cam, "focal_length_x")
                @test cam["focal_length_x"] == 700.0
            else
                @info "interop fixture not present; skipping fixture test"
            end
        end

        @testset "errors" begin
            @test_throws Ptiff.PTIFFError Ptiff.read_info(joinpath(dir, "nope.tif"))
        end
    end
end
