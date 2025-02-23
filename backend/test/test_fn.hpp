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
    Int env = 7;
    TypedClosure<Int, Int, Int> adder_fn{adder, env};
    ASSERT_EQ(adder_fn.call(4), 11);
}

Int call_closure(Fn f, Int a) {
    return std::bit_cast<TypedFn<Int, Int> *>(&f)->call(a);
}

TEST(TestClosure, TestFnCast) {
    Int env = 4;
    TypedClosure<Int, Int, Int> adder_fn{adder, env};
    ASSERT_EQ(call_closure(adder_fn, 7), 11);
}

Int foo(Int x, std::shared_ptr<TypedWeakFn<Int, Int>> env) {
    if (x <= 0) {
        return 0;
    } else {
        return env->lock().call(x - 1);
    }
}

TEST(TestClosure, TestRecursiveClosure) {
    TypedClosure<TypedWeakFn<Int, Int>, Int, Int> foo_fn(foo);
    foo_fn.env() = TypedWeakFn<Int, Int>(foo_fn);

    ASSERT_EQ(call_closure(foo_fn, 3), 0);
}
