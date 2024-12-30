#pragma once

#include "time/utils.hpp"

#include <chrono>

template <typename Rep, typename Period>
void sleep(const std::chrono::duration<Rep, Period> &duration) {
    auto start = time_utils::now();
    while (time_utils::now() - start < duration) {
    }
}
