#pragma once

#include "fn/fn.hpp"
#include "fn/predefined.hpp"

#include <gtest/gtest.h>

struct IdentityInt : ParametricFn<int, int> {
    void body() override { *ret = *std::get<0>(args); }
};

GTEST_TEST(FnTests, IdentityTest) {
    IdentityInt id{};
    int x = 5, r = 0;
    std::get<0>(id.args) = &x;
    id.ret = &r;
    ASSERT_EQ(r, 0);

    id.run();
    ASSERT_EQ(r, 5);
    ASSERT_EQ(x, 5);
}

GTEST_TEST(FnTests, PlusTest) {
    Plus__BuiltIn plus{};
    Int x = 5, y = 10, r = 0;
    std::get<0>(plus.args) = &x;
    std::get<1>(plus.args) = &y;
    plus.ret = &r;
    ASSERT_EQ(r, 0);

    plus.run();
    ASSERT_EQ(x, 5);
    ASSERT_EQ(y, 10);
    ASSERT_EQ(r, 15);
}

GTEST_TEST(FnTests, MinusTest) {
    Minus__BuiltIn minus{};
    Int x = 5, y = 10, r = 0;
    std::get<0>(minus.args) = &x;
    std::get<1>(minus.args) = &y;
    minus.ret = &r;
    ASSERT_EQ(r, 0);

    minus.run();
    ASSERT_EQ(x, 5);
    ASSERT_EQ(y, 10);
    ASSERT_EQ(r, -5);
}

struct FourWayPlus : ParametricFn<Int, Int, Int, Int, Int> {
    void body() override {
        Plus__BuiltIn *a = new Plus__BuiltIn{}, *b = new Plus__BuiltIn{};
        std::get<0>(a->args) = std::get<0>(args);
        std::get<1>(a->args) = std::get<1>(args);
        std::get<0>(b->args) = std::get<2>(args);
        std::get<1>(b->args) = std::get<3>(args);
        Int *x = new Int{}, *y = new Int{};
        a->ret = x;
        b->ret = y;

        Plus__BuiltIn *c = new Plus__BuiltIn{};
        std::get<0>(c->args) = x;
        std::get<1>(c->args) = y;
        c->ret = ret;

        a->conts = {c};
        b->conts = {c};
        c->deps = 2;

        a->run();
        b->run();
    }
};

GTEST_TEST(FnTests, FourWayPlusTest) {
    FourWayPlus plus{};
    Int w = 11, x = 5, y = 10, z = 22, r = 0;
    std::get<0>(plus.args) = &w;
    std::get<1>(plus.args) = &x;
    std::get<2>(plus.args) = &y;
    std::get<3>(plus.args) = &z;
    plus.ret = &r;
    ASSERT_EQ(r, 0);

    plus.run();
    ASSERT_EQ(w, 11);
    ASSERT_EQ(x, 5);
    ASSERT_EQ(y, 10);
    ASSERT_EQ(z, 22);
    ASSERT_EQ(r, 48);
}
