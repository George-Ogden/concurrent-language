#pragma once

#include "fn/fn.tpp"
#include "system/thread_manager.tpp"
#include "types/builtin.hpp"

#include <gtest/gtest.h>

#include <bit>
#include <memory>

Int plus(Int a, Int b, const std::shared_ptr<void> env) { return a + b; }

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

Int adder(Int a, const std::shared_ptr<Int> env) { return a + *env; }

TEST(TestClosure, TestClosureCall) {
    std::shared_ptr<Int> env = std::make_shared<Int>(7);
    TypedClosure<Int, Int, Int> adder_fn{adder, env};
    ASSERT_EQ((dynamic_cast<TypedFn<Int, Int> *>(&adder_fn)->call(4)), 11);
}

Int call_closure(Fn f, Int a) {
    return std::bit_cast<TypedFn<Int, Int> *>(&f)->call(a);
}

TEST(TestClosure, TestFnCast) {
    std::shared_ptr<Int> env = std::make_shared<Int>(4);
    TypedClosure<Int, Int, Int> adder_fn{adder, env};
    ASSERT_EQ(call_closure(adder_fn, 7), 11);
}
