#pragma once

#include "system/cache_utils.hpp"

#include <gtest/gtest.h>
#include <range/v3/view/zip.hpp>

#include <vector>

TEST(CacheInfoTest, CacheLineSizeTest) {
    ASSERT_EQ(cache_utils::get_line_size(1), 64);
    ASSERT_EQ(cache_utils::get_line_size(2), 256);
    ASSERT_EQ(cache_utils::get_line_size(3), 256);
    ASSERT_EQ(cache_utils::get_line_size(4), 0);
    ASSERT_EQ(cache_utils::get_line_size(5), 0);
}

TEST(CacheInfoTest, NumCacheLinesTest) {
    ASSERT_EQ(cache_utils::get_num_lines(1), 768);
    ASSERT_EQ(cache_utils::get_num_lines(2), 5120);
    ASSERT_EQ(cache_utils::get_num_lines(3), 16384);
    ASSERT_EQ(cache_utils::get_num_lines(4), 0);
    ASSERT_EQ(cache_utils::get_num_lines(5), 0);
}

TEST(CacheInfoTest, AssociativityTest) {
    ASSERT_EQ(cache_utils::get_associativity(1), 12);
    ASSERT_EQ(cache_utils::get_associativity(2), 16);
    ASSERT_EQ(cache_utils::get_associativity(3), 8);
    ASSERT_EQ(cache_utils::get_associativity(4), 0);
    ASSERT_EQ(cache_utils::get_associativity(5), 0);
}

TEST(CacheLevelTest, ChangingCacheLevelTest) {
    cache_utils::set_default_level(2);
    ASSERT_EQ(cache_utils::get_default_level(), 2);

    cache_utils::set_default_level(1);
    cache_utils::set_default_level(3);
    ASSERT_EQ(cache_utils::get_default_level(), 3);
}

TEST(CacheLevelTest, ChangeAttributeResults) {
    cache_utils::set_default_level(1);
    ASSERT_EQ(cache_utils::get_line_size(), cache_utils::get_line_size(1));
    ASSERT_EQ(cache_utils::get_cache_size(), cache_utils::get_cache_size(1));
    ASSERT_EQ(cache_utils::get_num_lines(), cache_utils::get_num_lines(1));
    ASSERT_EQ(cache_utils::get_associativity(),
              cache_utils::get_associativity(1));

    cache_utils::set_default_level(2);
    ASSERT_EQ(cache_utils::get_line_size(), cache_utils::get_line_size(2));
    ASSERT_EQ(cache_utils::get_cache_size(), cache_utils::get_cache_size(2));
    ASSERT_EQ(cache_utils::get_num_lines(), cache_utils::get_num_lines(2));
    ASSERT_EQ(cache_utils::get_associativity(),
              cache_utils::get_associativity(2));
}
