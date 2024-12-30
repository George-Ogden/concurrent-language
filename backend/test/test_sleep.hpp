#pragma once

#include "time/sleep.hpp"
#include "time/utils.hpp"

#include <gtest/gtest.h>

#include <chrono>

TEST(DISABLED_SleepTest, SleepsForTwoSeconds) {
    auto duration = std::chrono::seconds(2);
    auto start = time_utils::now();

    sleep(duration);

    auto elapsed = time_utils::now() - start;
    EXPECT_LT(elapsed, duration + std::chrono::milliseconds(100));
    EXPECT_GE(elapsed, duration);
}

TEST(DISABLED_SleepTest, SleepsFor500Milliseconds) {
    auto duration = std::chrono::milliseconds(500);
    auto start = time_utils::now();

    sleep(duration);

    auto elapsed = time_utils::now() - start;
    EXPECT_LT(elapsed, duration + std::chrono::milliseconds(50));
    EXPECT_GE(elapsed, duration);
}
