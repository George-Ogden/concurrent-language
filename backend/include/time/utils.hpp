#pragma once

#include <chrono>
#include <type_traits>

using namespace std::chrono_literals;

typedef std::chrono::duration<std::chrono::nanoseconds::rep,
                              std::chrono::nanoseconds::period>
    Duration;
using ClockType =
    std::conditional_t<std::chrono::high_resolution_clock::is_steady,
                       std::chrono::high_resolution_clock,
                       std::chrono::steady_clock>;
typedef std::chrono::time_point<ClockType> TimePoint;

namespace time_utils {
TimePoint now() { return ClockType::now(); }

Duration time_delta(const TimePoint &start, const TimePoint &end) {
    return std::chrono::duration_cast<std::chrono::nanoseconds>(end - start);
}
} // namespace time_utils
