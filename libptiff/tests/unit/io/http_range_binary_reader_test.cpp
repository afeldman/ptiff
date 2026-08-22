#include <algorithm>
#include <array>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <string>
#include <thread>
#include <vector>

#include <httplib.h>

#include <ptiff/io/http_range_binary_reader.hpp>

#include <catch2/catch_test_macros.hpp>

using ptiff::ErrorCode;
using ptiff::io::HttpRangeBinaryReader;

namespace {

// A loopback-only HTTP server serving a fixed in-memory "object" with real Range semantics,
// so HttpRangeBinaryReader is tested against genuine HTTP responses without touching any real
// network or cloud provider (tests must not depend on external services).
class LocalRangeServer {
public:
    explicit LocalRangeServer(std::size_t objectSize) : object_(objectSize) {
        for (std::size_t i = 0; i < object_.size(); ++i) {
            object_[i] = static_cast<char>(i % 251);
        }

        server_.Get("/object", [this](const httplib::Request& req, httplib::Response& res) {
            ++requestCount_;
            lastAuthHeader_ = req.get_header_value("Authorization");
            handleObjectRequest(req, res);
        });
        server_.Get("/missing",
                    [](const httplib::Request&, httplib::Response& res) { res.status = 404; });

        port_ = server_.bind_to_any_port("127.0.0.1");
        thread_ = std::thread([this] { server_.listen_after_bind(); });
        while (!server_.is_running()) {
            std::this_thread::yield();
        }
    }

    ~LocalRangeServer() {
        server_.stop();
        thread_.join();
    }

    LocalRangeServer(const LocalRangeServer&) = delete;
    LocalRangeServer& operator=(const LocalRangeServer&) = delete;

    [[nodiscard]] std::string objectUrl() const {
        return "http://127.0.0.1:" + std::to_string(port_) + "/object";
    }
    [[nodiscard]] std::string missingUrl() const {
        return "http://127.0.0.1:" + std::to_string(port_) + "/missing";
    }
    [[nodiscard]] int requestCount() const { return requestCount_; }
    [[nodiscard]] const std::string& lastAuthHeader() const { return lastAuthHeader_; }

private:
    void handleObjectRequest(const httplib::Request& req, httplib::Response& res) {
        // This handler implements Range semantics itself (status, Content-Range, and a
        // pre-clamped body) so the test exercises HttpRangeBinaryReader against a realistic
        // server. cpp-httplib also does its own automatic post-processing of any request that
        // carried a Range header (comparing the *raw* requested range against the response body
        // it sees), which would otherwise re-validate against the unclamped range and clobber the
        // status this handler just set (e.g. forcing a spurious 416). Clearing `ranges` here
        // opts this response out of that automatic post-processing so the status/headers/body set
        // below are what actually goes out on the wire.
        const_cast<httplib::Request&>(req).ranges.clear();

        const auto rangeHeader = req.get_header_value("Range");
        if (rangeHeader.empty()) {
            res.status = 200;
            res.set_content(object_.data(), object_.size(), "application/octet-stream");
            return;
        }

        std::uint64_t start = 0;
        std::uint64_t end = 0;
        // Format is always "bytes=START-END" here -- HttpRangeBinaryReader always sends both
        // bounds explicitly, so a minimal parse is sufficient for this test server.
        const auto dashPos = rangeHeader.find('-', rangeHeader.find('='));
        start = std::stoull(rangeHeader.substr(rangeHeader.find('=') + 1, dashPos));
        end = std::stoull(rangeHeader.substr(dashPos + 1));

        if (object_.empty() || start >= object_.size()) {
            res.status = 416;
            return;
        }
        end = std::min<std::uint64_t>(end, object_.size() - 1);

        res.status = 206;
        res.set_header("Content-Range",
                       "bytes " + std::to_string(start) + "-" + std::to_string(end) + "/" +
                           std::to_string(object_.size()));
        res.set_content(object_.data() + start,
                        static_cast<std::size_t>(end - start + 1),
                        "application/octet-stream");
    }

    std::vector<char> object_;
    httplib::Server server_;
    std::thread thread_;
    int port_ = 0;
    std::atomic<int> requestCount_{0};
    std::string lastAuthHeader_;
};

} // namespace

TEST_CASE("HttpRangeBinaryReader opens a small object via a 206 response",
          "[http-range-binary-reader]") {
    LocalRangeServer server(1000);
    auto reader = HttpRangeBinaryReader::open(server.objectUrl());
    REQUIRE(reader.has_value());
    REQUIRE(reader.value()->size().value() == 1000);
    REQUIRE(reader.value()->position().value() == 0);
}

TEST_CASE("HttpRangeBinaryReader reads sequentially matching the object's bytes",
          "[http-range-binary-reader]") {
    LocalRangeServer server(1000);
    auto reader = HttpRangeBinaryReader::open(server.objectUrl());
    REQUIRE(reader.has_value());

    std::array<std::byte, 10> buf{};
    auto n = reader.value()->read(buf);
    REQUIRE(n.has_value());
    REQUIRE(*n == 10);
    for (std::size_t i = 0; i < 10; ++i) {
        REQUIRE(buf[i] == static_cast<std::byte>(i % 251));
    }
    REQUIRE(reader.value()->position().value() == 10);
}

TEST_CASE("HttpRangeBinaryReader coalesces small sequential reads into the read-ahead buffer",
          "[http-range-binary-reader]") {
    LocalRangeServer server(1000);
    auto reader = HttpRangeBinaryReader::open(server.objectUrl());
    REQUIRE(reader.has_value());
    const int requestsAfterOpen = server.requestCount();

    std::array<std::byte, 8> buf{};
    for (int i = 0; i < 20; ++i) {
        auto n = reader.value()->read(buf);
        REQUIRE(n.has_value());
        REQUIRE(*n == 8);
    }

    // 20 * 8 = 160 bytes, well inside the object (1000B) and the 64KiB read-ahead buffer primed
    // at open() -- every one of these reads must be served from that buffer.
    REQUIRE(server.requestCount() == requestsAfterOpen);
}

TEST_CASE("HttpRangeBinaryReader seek past the buffer triggers a new fetch",
          "[http-range-binary-reader]") {
    LocalRangeServer server(200000);
    auto reader = HttpRangeBinaryReader::open(server.objectUrl());
    REQUIRE(reader.has_value());
    const int requestsAfterOpen = server.requestCount();

    REQUIRE(reader.value()->seek(150000).has_value());
    std::array<std::byte, 4> buf{};
    auto n = reader.value()->read(buf);
    REQUIRE(n.has_value());
    REQUIRE(*n == 4);
    for (std::size_t i = 0; i < 4; ++i) {
        REQUIRE(buf[i] == static_cast<std::byte>((150000 + i) % 251));
    }
    REQUIRE(server.requestCount() == requestsAfterOpen + 1);
}

TEST_CASE("HttpRangeBinaryReader an oversized read does not evict the read-ahead buffer",
          "[http-range-binary-reader]") {
    LocalRangeServer server(300000);
    auto reader = HttpRangeBinaryReader::open(server.objectUrl());
    REQUIRE(reader.has_value());

    // First, touch the start of the buffer primed at open() so we know it is populated.
    std::array<std::byte, 4> small{};
    REQUIRE(reader.value()->read(small).has_value());

    // Now seek far away and issue a read bigger than the 64KiB buffer -- this must not replace
    // the buffer that still covers [0, 64KiB).
    REQUIRE(reader.value()->seek(100000).has_value());
    std::vector<std::byte> big(100000);
    auto bigRead = reader.value()->read(big);
    REQUIRE(bigRead.has_value());
    REQUIRE(*bigRead == 100000);

    // Seeking back to just after the earlier small read must not trigger a new request: the
    // original buffer should still be intact.
    REQUIRE(reader.value()->seek(4).has_value());
    const int requestsBeforeFinalRead = server.requestCount();
    std::array<std::byte, 4> again{};
    auto n = reader.value()->read(again);
    REQUIRE(n.has_value());
    REQUIRE(*n == 4);
    REQUIRE(server.requestCount() == requestsBeforeFinalRead);
}

TEST_CASE("HttpRangeBinaryReader sends the bearer token header when provided",
          "[http-range-binary-reader]") {
    LocalRangeServer server(100);
    auto reader = HttpRangeBinaryReader::open(server.objectUrl(), "test-token-123");
    REQUIRE(reader.has_value());
    REQUIRE(server.lastAuthHeader() == "Bearer test-token-123");
}

TEST_CASE("HttpRangeBinaryReader open without a bearer token sends no Authorization header",
          "[http-range-binary-reader]") {
    LocalRangeServer server(100);
    auto reader = HttpRangeBinaryReader::open(server.objectUrl());
    REQUIRE(reader.has_value());
    REQUIRE(server.lastAuthHeader().empty());
}

TEST_CASE("HttpRangeBinaryReader open on a 404 URL fails with NotFound",
          "[http-range-binary-reader]") {
    LocalRangeServer server(100);
    auto reader = HttpRangeBinaryReader::open(server.missingUrl());
    REQUIRE_FALSE(reader.has_value());
    REQUIRE(reader.error().code() == ErrorCode::NotFound);
}

TEST_CASE("HttpRangeBinaryReader open on an empty object (416 probe) reports size zero",
          "[http-range-binary-reader]") {
    LocalRangeServer server(0);
    auto reader = HttpRangeBinaryReader::open(server.objectUrl());
    REQUIRE(reader.has_value());
    REQUIRE(reader.value()->size().value() == 0);

    std::array<std::byte, 4> buf{};
    auto n = reader.value()->read(buf);
    REQUIRE(n.has_value());
    REQUIRE(*n == 0);
}

TEST_CASE("HttpRangeBinaryReader seek beyond the known size fails with OutOfRange",
          "[http-range-binary-reader]") {
    LocalRangeServer server(100);
    auto reader = HttpRangeBinaryReader::open(server.objectUrl());
    REQUIRE(reader.has_value());
    auto result = reader.value()->seek(1000);
    REQUIRE_FALSE(result.has_value());
    REQUIRE(result.error().code() == ErrorCode::OutOfRange);
}

TEST_CASE("HttpRangeBinaryReader open rejects a url without an http/https scheme",
          "[http-range-binary-reader]") {
    auto reader = HttpRangeBinaryReader::open("ftp://example.com/object");
    REQUIRE_FALSE(reader.has_value());
    REQUIRE(reader.error().code() == ErrorCode::InvalidArgument);
}
