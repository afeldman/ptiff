#include <algorithm>
#include <cctype>
#include <charconv>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include <curl/curl.h>

#include <ptiff/io/http_range_binary_reader.hpp>

namespace ptiff::io {

namespace {

constexpr std::size_t kDefaultBufferSize = 65536;

// curl_global_init/curl_global_cleanup are not reference-counted and are documented as unsafe to
// call concurrently; this guard runs curl_global_init at most once per process. Deliberately no
// matching curl_global_cleanup: libcurl's own docs say cleanup must happen exactly once, after
// every other libcurl use in the process (including on other threads) has finished, which a
// library has no reliable way to guarantee -- skipping it is the standard, safe choice for
// library code (the OS reclaims the resources at process exit either way).
void ensureCurlGlobalInit() {
    static const int guard = [] {
        curl_global_init(CURL_GLOBAL_DEFAULT);
        return 0;
    }();
    (void)guard;
}

std::size_t curlWriteCallback(char* ptr, std::size_t size, std::size_t nmemb, void* userdata) {
    auto* buf = static_cast<std::vector<std::byte>*>(userdata);
    const std::size_t bytes = size * nmemb;
    const auto* src = reinterpret_cast<const std::byte*>(ptr);
    buf->insert(buf->end(), src, src + bytes);
    return bytes;
}

// Scans one response header line for "Content-Range: bytes START-END/TOTAL" and records TOTAL.
// libcurl has no built-in accessor for an arbitrary response header's value across all supported
// versions, so this is a manual CURLOPT_HEADERFUNCTION callback rather than a version-specific
// convenience API.
std::size_t curlHeaderCallback(char* buffer, std::size_t size, std::size_t nitems, void* userdata) {
    auto* totalSize = static_cast<std::optional<std::uint64_t>*>(userdata);
    const std::size_t len = size * nitems;
    const std::string_view line(buffer, len);

    constexpr std::string_view kPrefix = "content-range:";
    if (line.size() <= kPrefix.size()) {
        return len;
    }
    std::string lowered(line.substr(0, kPrefix.size()));
    std::ranges::transform(lowered, lowered.begin(), [](unsigned char c) {
        return static_cast<char>(std::tolower(c));
    });
    if (lowered != kPrefix) {
        return len;
    }

    const auto slashPos = line.rfind('/');
    if (slashPos == std::string_view::npos) {
        return len;
    }
    const auto totalStr = line.substr(slashPos + 1);
    std::uint64_t value = 0;
    const auto parsed = std::from_chars(totalStr.data(), totalStr.data() + totalStr.size(), value);
    if (parsed.ec == std::errc{}) {
        *totalSize = value;
    }
    return len;
}

struct RangedGetResult {
    long status = 0;
    std::optional<std::uint64_t> totalSize;
    std::vector<std::byte> body;
};

Result<RangedGetResult> performRangedGet(CURL* curl,
                                         const std::string& url,
                                         const std::string& authHeader,
                                         std::uint64_t start,
                                         std::uint64_t length) {
    RangedGetResult result;
    const std::string rangeHeader =
        "Range: bytes=" + std::to_string(start) + "-" + std::to_string(start + length - 1);

    curl_slist* headers = curl_slist_append(nullptr, rangeHeader.c_str());
    if (!authHeader.empty()) {
        headers = curl_slist_append(headers, authHeader.c_str());
    }

    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_HTTPGET, 1L);
    curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, curlWriteCallback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &result.body);
    curl_easy_setopt(curl, CURLOPT_HEADERFUNCTION, curlHeaderCallback);
    curl_easy_setopt(curl, CURLOPT_HEADERDATA, &result.totalSize);
    curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    curl_easy_setopt(curl, CURLOPT_NOSIGNAL, 1L);
    curl_easy_setopt(curl, CURLOPT_CONNECTTIMEOUT_MS, 10000L);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT_MS, 30000L);

    const CURLcode rc = curl_easy_perform(curl);
    curl_slist_free_all(headers);

    if (rc != CURLE_OK) {
        return std::unexpected(Error{
            ErrorCode::Unknown, std::string{"HttpRangeBinaryReader: "} + curl_easy_strerror(rc)});
    }

    long status = 0;
    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &status);
    result.status = status;
    return result;
}

} // namespace

struct HttpRangeBinaryReader::Impl {
    CURL* curl = nullptr;
    std::string url;
    std::string authHeader;
    std::uint64_t totalSize = 0;
    std::uint64_t cursor = 0;
    std::vector<std::byte> buffer;
    std::uint64_t bufferStart = 0;

    ~Impl() {
        if (curl != nullptr) {
            curl_easy_cleanup(curl);
        }
    }
};

HttpRangeBinaryReader::HttpRangeBinaryReader() : impl_(std::make_unique<Impl>()) {}

HttpRangeBinaryReader::~HttpRangeBinaryReader() = default;

Result<std::unique_ptr<HttpRangeBinaryReader>>
HttpRangeBinaryReader::open(const std::string& url, std::string_view bearerToken) {
    if (!url.starts_with("http://") && !url.starts_with("https://")) {
        return std::unexpected(
            Error{ErrorCode::InvalidArgument,
                  "HttpRangeBinaryReader::open: url must start with http:// or https://: " + url});
    }

    ensureCurlGlobalInit();

    std::unique_ptr<HttpRangeBinaryReader> reader{new HttpRangeBinaryReader()};
    reader->impl_->curl = curl_easy_init();
    if (reader->impl_->curl == nullptr) {
        return std::unexpected(
            Error{ErrorCode::Unknown, "HttpRangeBinaryReader::open: curl_easy_init failed"});
    }
    reader->impl_->url = url;
    if (!bearerToken.empty()) {
        reader->impl_->authHeader = "Authorization: Bearer " + std::string{bearerToken};
    }

    auto probe = performRangedGet(
        reader->impl_->curl, reader->impl_->url, reader->impl_->authHeader, 0, kDefaultBufferSize);
    if (!probe.has_value()) {
        return std::unexpected(probe.error());
    }

    if (probe->status == 206) {
        if (!probe->totalSize.has_value()) {
            return std::unexpected(
                Error{ErrorCode::Unknown,
                      "HttpRangeBinaryReader::open: 206 response missing Content-Range total "
                      "size: " +
                          url});
        }
        reader->impl_->totalSize = *probe->totalSize;
        reader->impl_->buffer = std::move(probe->body);
        reader->impl_->bufferStart = 0;
    } else if (probe->status == 200) {
        reader->impl_->totalSize = probe->body.size();
        reader->impl_->buffer = std::move(probe->body);
        reader->impl_->bufferStart = 0;
    } else if (probe->status == 416) {
        reader->impl_->totalSize = 0;
    } else if (probe->status == 404) {
        return std::unexpected(
            Error{ErrorCode::NotFound, "HttpRangeBinaryReader::open: 404 Not Found: " + url});
    } else {
        return std::unexpected(Error{ErrorCode::Unknown,
                                     "HttpRangeBinaryReader::open: unexpected HTTP status " +
                                         std::to_string(probe->status) + ": " + url});
    }

    return reader;
}

Result<std::size_t> HttpRangeBinaryReader::read(std::span<std::byte> destination) {
    if (destination.empty()) {
        return std::size_t{0};
    }
    if (impl_->cursor >= impl_->totalSize) {
        return std::size_t{0};
    }

    const std::uint64_t remaining = impl_->totalSize - impl_->cursor;
    const std::size_t wanted =
        static_cast<std::size_t>(std::min<std::uint64_t>(destination.size(), remaining));

    const bool hit = impl_->cursor >= impl_->bufferStart &&
                     impl_->cursor + wanted <= impl_->bufferStart + impl_->buffer.size();
    if (hit) {
        const std::size_t offset = static_cast<std::size_t>(impl_->cursor - impl_->bufferStart);
        std::copy_n(impl_->buffer.begin() + static_cast<std::ptrdiff_t>(offset),
                    wanted,
                    destination.begin());
        impl_->cursor += wanted;
        return wanted;
    }

    const std::uint64_t fetchLen =
        std::min<std::uint64_t>(std::max<std::uint64_t>(wanted, kDefaultBufferSize), remaining);
    auto fetched =
        performRangedGet(impl_->curl, impl_->url, impl_->authHeader, impl_->cursor, fetchLen);
    if (!fetched.has_value()) {
        return std::unexpected(fetched.error());
    }
    if (fetched->status == 416) {
        // Only reachable if the remote object shrank between open() and this read() -- the
        // range requested here is always clamped to what open() learned was available.
        return std::unexpected(Error{ErrorCode::OutOfRange,
                                     "HttpRangeBinaryReader::read: range no longer satisfiable "
                                     "(416) -- remote object may have changed since open()"});
    }
    if (fetched->status != 206 && fetched->status != 200) {
        return std::unexpected(Error{ErrorCode::Unknown,
                                     "HttpRangeBinaryReader::read: unexpected HTTP status " +
                                         std::to_string(fetched->status)});
    }

    if (fetched->body.size() > kDefaultBufferSize) {
        // Oversized fetch (a single read wider than the buffer): serve directly, leave the
        // read-ahead buffer untouched so it still covers whatever it covered before.
        const std::size_t n = std::min(wanted, fetched->body.size());
        std::copy_n(fetched->body.begin(), n, destination.begin());
        impl_->cursor += n;
        return n;
    }

    impl_->buffer = std::move(fetched->body);
    impl_->bufferStart = impl_->cursor;
    const std::size_t n = std::min<std::size_t>(wanted, impl_->buffer.size());
    std::copy_n(impl_->buffer.begin(), n, destination.begin());
    impl_->cursor += n;
    return n;
}

Result<void> HttpRangeBinaryReader::seek(std::uint64_t offset) {
    if (offset > impl_->totalSize) {
        return std::unexpected(
            Error{ErrorCode::OutOfRange, "HttpRangeBinaryReader::seek: offset beyond size"});
    }
    impl_->cursor = offset;
    return {};
}

Result<std::uint64_t> HttpRangeBinaryReader::position() const {
    return impl_->cursor;
}

Result<std::uint64_t> HttpRangeBinaryReader::size() const {
    return impl_->totalSize;
}

} // namespace ptiff::io
