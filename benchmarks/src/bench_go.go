// Command bench_go measures read/write throughput of the SWIG Go binding
// (ptiff) with the same metric set and median-of-repeats methodology as the
// Python/Ruby/Octave benchmarks (see benchmarks/src/bench_python.py).
//
// Driver: `go run ./src/bench_go.go --out <json>` from within benchmarks/
// (uses the replace directive in go.mod to pull github.com/crater/ptiff-go).
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"time"

	ptiff "github.com/crater/ptiff-go/ptiff"
)

const (
	writeSize = 128
	writeTile = 64
)

var (
	repeats  = envInt("BENCH_REPEATS", 20)
	iters    = envInt("BENCH_ITERS", 50)
	nacIters = envInt("BENCH_NAC_ITERS", 2)
)

func envInt(name string, def int) int {
	if v, ok := os.LookupEnv(name); ok {
		var n int
		if _, err := fmt.Sscanf(v, "%d", &n); err == nil {
			return n
		}
	}
	return def
}

func fixturesDir() string {
	// Benchmarks run from benchmarks/ (see run_benchmarks.sh); fixtures live in
	// benchmarks/fixtures. Resolve relative to os.Getwd() so the binary works
	// regardless of how it is invoked, as long as it is launched from a
	// descendant of benchmarks/.
	cwd, err := os.Getwd()
	if err == nil {
		if _, derr := os.Stat(filepath.Join(cwd, "fixtures")); derr == nil {
			return filepath.Join(cwd, "fixtures")
		}
		// launched from repo root: benchmarks/fixtures
		if fi, derr := os.Stat(filepath.Join(cwd, "benchmarks", "fixtures")); derr == nil && fi.IsDir() {
			return filepath.Join(cwd, "benchmarks", "fixtures")
		}
	}
	return "fixtures"
}

func newDesc(width, height uint, pixelType int, tile uint) ptiff.Ptiff_image_descriptor {
	d := ptiff.NewPtiff_image_descriptor()
	d.SetWidth(width)
	d.SetHeight(height)
	d.SetPixel_type(pixelType)
	d.SetChannel_count(1)
	d.SetHas_tile_info(1)
	t := ptiff.NewPtiff_tile_info()
	t.SetTile_width(tile)
	t.SetTile_height(tile)
	d.SetTile_info(t)
	d.SetHas_compression(0)
	return d
}

type sample struct {
	Repeats  int     `json:"repeats"`
	MedianMs float64 `json:"median_ms"`
	MinMs    float64 `json:"min_ms"`
	MaxMs    float64 `json:"max_ms"`
}

func medianTimes(repeats int, fn func()) sample {
	times := make([]float64, 0, repeats)
	for i := 0; i < repeats; i++ {
		t0 := time.Now()
		fn()
		times = append(times, float64(time.Since(t0).Microseconds())/1000.0)
	}
	sort.Float64s(times)
	return sample{
		Repeats:  repeats,
		MedianMs: round4(times[len(times)/2]),
		MinMs:    round4(times[0]),
		MaxMs:    round4(times[len(times)-1]),
	}
}

func round4(v float64) float64 { return float64(int(v*10000+0.5)) / 10000 }

func writeAllTiles(outPath string, iters int) {
	d := newDesc(uint(writeSize), uint(writeSize), 0, uint(writeTile))
	_ = os.Remove(outPath)
	sink := ptiff.Ptiff_sink_create(outPath, d)
	cols := ptiff.Ptiff_sink_tile_columns(sink)
	rows := ptiff.Ptiff_sink_tile_rows(sink)
	bs := ptiff.Ptiff_sink_tile_byte_size(sink)
	pat := make([]byte, bs)
	for i := range pat {
		pat[i] = 7
	}
	for i := 0; i < iters; i++ {
		for c := uint(0); c < cols; c++ {
			for r := uint(0); r < rows; r++ {
				if rc := ptiff.Ptiff_sink_write_tile(sink, c, r, pat); rc != 0 {
					panic(fmt.Sprintf("write_tile(%d,%d) rc=%d", c, r, rc))
				}
			}
		}
	}
	ptiff.Ptiff_sink_close(sink)
}

func readAllTiles(path string, iters int) {
	var errOut int
	src := ptiff.Ptiff_source_open(path, &errOut)
	ncols := ptiff.Ptiff_source_tile_columns(src)
	nrows := ptiff.Ptiff_source_tile_rows(src)
	bs := ptiff.Ptiff_source_tile_byte_size(src)
	for i := 0; i < iters; i++ {
		buf := make([]byte, bs)
		for c := uint(0); c < ncols; c++ {
			for r := uint(0); r < nrows; r++ {
				var nread uint
				if rc := ptiff.Ptiff_source_read_tile(src, c, r, buf, &nread); rc != 0 {
					panic(fmt.Sprintf("read_tile(%d,%d) rc=%d", c, r, rc))
				}
			}
		}
	}
	ptiff.Ptiff_source_close(src)
}

func fixtureInfo(name string) map[string]any {
	p := filepath.Join(fixturesDir(), name)
	st, err := os.Stat(p)
	size := int64(-1)
	if err == nil {
		size = st.Size()
	}
	return map[string]any{"file": p, "size_bytes": size, "exists": err == nil}
}

func main() {
	var outPath string
	var useReal bool
	var useNac bool
	flag.StringVar(&outPath, "out", "", "output JSON path")
	flag.BoolVar(&useReal, "real", false, "also read fixtures/real_lola_512.tif (real NASA LOLA crop) if present")
	flag.BoolVar(&useNac, "nac", false, "also read fixtures/nac_dtm.tif (real NASA LRO-NAC DTM, 616 tiles) if present")
	flag.Parse()
	if outPath == "" {
		fmt.Fprintln(os.Stderr, "missing --out")
		os.Exit(2)
	}

	// warm-up
	newDesc(1, 1, 0, 1)

	tmp, err := os.MkdirTemp("", "ptiff-bench-")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(tmp)

	writeAllTiles(filepath.Join(tmp, "write_small.tif"), 1)

	metrics := map[string]sample{
		"write_all_tiles_ms": medianTimes(repeats, func() { writeAllTiles(filepath.Join(tmp, "write_small.tif"), iters) }),
		"read_uint8_128_ms":  medianTimes(repeats, func() { readAllTiles(filepath.Join(fixturesDir(), "uint8_128.tif"), iters) }),
		"read_uint8_512_ms":  medianTimes(repeats, func() { readAllTiles(filepath.Join(fixturesDir(), "uint8_512.tif"), iters) }),
		"read_f32_512_ms":    medianTimes(repeats, func() { readAllTiles(filepath.Join(fixturesDir(), "f32_512.tif"), iters) }),
	}

	fix := fixturesDir()
	realTif := filepath.Join(fix, "real_lola_512.tif")
	nacTif := filepath.Join(fix, "nac_dtm.tif")
	if useReal {
		if _, err := os.Stat(realTif); err == nil {
			// --real: read a genuine NASA LOLA elevation crop (copied into
			// benchmarks/fixtures by run_benchmarks.sh --real).
			metrics["read_real_lola_512_ms"] = medianTimes(repeats, func() { readAllTiles(realTif, iters) })
		} else {
			fmt.Fprintln(os.Stderr, "[go] --real requested but real_lola_512.tif missing; skipping")
		}
	}
	if useNac {
		if _, err := os.Stat(nacTif); err == nil {
			// --nac: read the full real NASA LRO-NAC DTM (616 tiles/pass).
			metrics["read_nac_ms"] = medianTimes(repeats, func() { readAllTiles(nacTif, nacIters) })
		} else {
			fmt.Fprintln(os.Stderr, "[go] --nac requested but nac_dtm.tif missing; skipping")
		}
	}

	v := ptiff.Ptiff_runtime_version()
	fixtures := map[string]any{
		"uint8_128": fixtureInfo("uint8_128.tif"),
		"uint8_512": fixtureInfo("uint8_512.tif"),
		"f32_512":   fixtureInfo("f32_512.tif"),
	}
	if useReal {
		if _, err := os.Stat(realTif); err == nil {
			fixtures["real_lola_512"] = fixtureInfo("real_lola_512.tif")
		}
	}
	if useNac {
		if _, err := os.Stat(nacTif); err == nil {
			fixtures["nac_dtm"] = fixtureInfo("nac_dtm.tif")
		}
	}
	doc := map[string]any{
		"language":              "go",
		"binding_version":       fmt.Sprintf("%d.%d.%d", v.GetMajor(), v.GetMinor(), v.GetPatch()),
		"repeats":               repeats,
		"iterations_per_sample": iters,
		"write_image":           map[string]any{"width": writeSize, "height": writeSize, "pixel_type": "uint8", "tile": writeTile},
		"fixtures":              fixtures,
		"metrics":               metrics,
	}
	if useNac {
		if _, err := os.Stat(nacTif); err == nil {
			doc["nac_ifd"] = map[string]any{"tiles_per_pass": 616, "nac_iters": nacIters}
		}
	}

	raw, _ := json.MarshalIndent(doc, "", "  ")
	_ = os.MkdirAll(filepath.Dir(outPath), 0o755)
	_ = os.WriteFile(outPath, append(raw, '\n'), 0o644)
	fmt.Println(string(mustJSON(map[string]any{"language": "go", "metrics": metrics})))
}

func mustJSON(v any) []byte {
	b, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	return b
}
