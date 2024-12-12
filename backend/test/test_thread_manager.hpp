#pragma once

#include "system/thread_manager.hpp"

#include <gtest/gtest.h>

TEST(ThreadManagerTest, SingleThreadIdConversion) {
    ThreadManager::register_self(0);
    ASSERT_EQ(ThreadManager::get_id(), 0);
}

TEST(ThreadManagerTest, MultiThreadIdConversion) {
    std::thread t1([&]() {
        ThreadManager::register_self(0);
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
        ASSERT_EQ(ThreadManager::get_id(), 0);
    });

    std::thread t2([&]() {
        std::this_thread::sleep_for(std::chrono::milliseconds(5));
        ThreadManager::register_self(1);
        std::this_thread::sleep_for(std::chrono::milliseconds(5));
        ASSERT_EQ(ThreadManager::get_id(), 1);
    });

    t1.join();
    t2.join();
}

TEST(ThreadManagerTest, NumCpusOverride) {
    ThreadManager::reset_concurrency_override();
    ASSERT_EQ(ThreadManager::available_concurrency(),
              std::thread::hardware_concurrency());
    ThreadManager::override_concurrency(16);
    ASSERT_EQ(ThreadManager::available_concurrency(), 16);
    ThreadManager::override_concurrency(3);
    ASSERT_EQ(ThreadManager::available_concurrency(), 3);
    ThreadManager::reset_concurrency_override();
    ASSERT_EQ(ThreadManager::available_concurrency(),
              std::thread::hardware_concurrency());
}
