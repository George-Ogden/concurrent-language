#pragma once

#include "fn/fn.tpp"
#include "system/thread_manager.tpp"
#include "types/builtin.hpp"

#include <gtest/gtest.h>

#include <bit>

Int plus(Int a, Int b, std::shared_ptr<void> env) { return a + b; }

Int call_fn(Fn f, Int a, Int b) {
    return std::bit_cast<TypedFn<Int, Int, Int> *>(&f)->call(a, b);
}

TEST(TestFn, TestFnCall) {
    TypedFn<Int, Int, Int> plus_fn{plus, nullptr};
    ASSERT_EQ(plus_fn.call(3, 4), 7);
}

TEST(TestFn, TestFnCast) {
    TypedFn<Int, Int, Int> plus_fn{plus, nullptr};
    ASSERT_EQ(call_fn(plus_fn, 4, 3), 7);
}
