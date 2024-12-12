#pragma once

#include "fn/fn.hpp"
#include "fn/predefined.hpp"

#include <gtest/gtest.h>

#include <utility>
#include <vector>

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

struct BranchingExample : ParametricFn<Int, Int, Int, Int> {
    void body() override {
        Int x = *std::get<0>(args);
        Int *y = std::get<1>(args);
        Int *z = std::get<2>(args);
        Int *t = new Int{};

        Minus__BuiltIn *post_branch = new Minus__BuiltIn{};
        post_branch->args = std::make_tuple(t, new Int{2});
        post_branch->ret = ret;
        post_branch->deps = 1;

        Plus__BuiltIn *positive_branch = new Plus__BuiltIn;
        positive_branch->args = std::make_tuple(y, new Int{1});
        positive_branch->ret = t;
        positive_branch->conts = {post_branch};

        Plus__BuiltIn *negative_branch = new Plus__BuiltIn;
        negative_branch->args = std::make_tuple(z, new Int{1});
        negative_branch->ret = t;
        negative_branch->conts = {post_branch};

        if (x >= 0) {
            positive_branch->run();
        } else {
            negative_branch->run();
        }
    }
};

GTEST_TEST(FnTests, PositiveBranchingExampleTest) {
    BranchingExample branching{};
    Int x = 5, y = 10, z = 22, r = 0;
    branching.args = std::make_tuple(&x, &y, &z);
    branching.ret = &r;
    ASSERT_EQ(r, 0);

    branching.run();
    ASSERT_EQ(x, 5);
    ASSERT_EQ(y, 10);
    ASSERT_EQ(z, 22);
    ASSERT_EQ(r, 9);
}

GTEST_TEST(FnTests, NegativeBranchingExampleTest) {
    BranchingExample branching{};
    Int x = -5, y = 10, z = 22, r = 0;
    branching.args = std::make_tuple(&x, &y, &z);
    branching.ret = &r;
    ASSERT_EQ(r, 0);

    branching.run();
    ASSERT_EQ(x, -5);
    ASSERT_EQ(y, 10);
    ASSERT_EQ(z, 22);
    ASSERT_EQ(r, 21);
}

struct EvenOrOdd : ParametricFn<Bool, Int> {
    void body() override { *ret = static_cast<bool>(*std::get<0>(args) & 1); }
};

struct ApplyIntBool : ParametricFn<Bool, ParametricFn<Bool, Int>, Int> {
    void body() override {
        ParametricFn<Bool, Int> *f = std::get<0>(args);
        Int *x = std::get<1>(args);
        f->args = {x};
        f->ret = ret;
        f->run();
    }
};

GTEST_TEST(FnTests, HigherOrderFunctionTest) {
    ApplyIntBool apply{};
    EvenOrOdd f{};
    Int x = 5;
    Bool r = false;
    apply.args = std::make_tuple(&f, &x);
    apply.ret = &r;
    ASSERT_FALSE(r);

    apply.run();
    ASSERT_EQ(x, 5);
    ASSERT_TRUE(r);
}

struct PairIntBool : ParametricFn<std::tuple<Int, Bool>, Int, Bool> {
    void body() {
        *ret = std::make_tuple(*std::get<0>(args), *std::get<1>(args));
    }
};

GTEST_TEST(FnTests, TupleTest) {
    PairIntBool pair{};
    Int x = 5;
    Bool y = true;
    Tuple<Int, Bool> r;
    pair.args = std::make_tuple(&x, &y);
    pair.ret = &r;

    pair.run();
    ASSERT_EQ(x, 5);
    ASSERT_TRUE(y);
    ASSERT_EQ(r, std::make_tuple(x, y));
}

using Bull = Variant<std::monostate, std::monostate>;

struct BoolUnion : ParametricFn<Bool, Bull> {
    void body() { *ret = std::get<0>(args)->tag == 0; }
};

GTEST_TEST(FnTests, ValueFreeUnionTest) {
    {
        BoolUnion fn{};
        Bool r;
        Bull bull{};
        bull.tag = 0;
        std::get<0>(fn.args) = &bull;
        fn.ret = &r;

        fn.run();
        ASSERT_TRUE(r);
    }

    {
        BoolUnion fn{};
        Bool r;
        Bull bull{};
        bull.tag = 1;
        std::get<0>(fn.args) = &bull;
        fn.ret = &r;

        fn.run();
        ASSERT_FALSE(r);
    }
}

using EitherIntBool = Variant<Int, Bool>;

struct EitherIntBoolExtractor : ParametricFn<Bool, EitherIntBool> {
    void body() {
        EitherIntBool tagged_union = *std::get<0>(args);
        switch (tagged_union.tag) {
        case 0:
            *ret = *reinterpret_cast<int *>(&tagged_union.value) > 10;
            break;
        case 1:
            *ret = *reinterpret_cast<bool *>(&tagged_union.value);
            break;
        }
    }
};

GTEST_TEST(FnTests, ValueIncludedUnionTest) {
    for (const auto &[tag, value, result] :
         std::vector<std::tuple<int, int, bool>>{{1, 0, false},
                                                 {1, 1, true},
                                                 {0, 0, false},
                                                 {0, 5, false},
                                                 {0, 15, true}}) {
        EitherIntBool either{};
        either.tag = tag;
        if (tag == 0) {

            *reinterpret_cast<int *>(&either.value) = value;
        } else {
            *reinterpret_cast<bool *>(&either.value) = value;
        }

        EitherIntBoolExtractor fn{};
        Bool r;
        std::get<0>(fn.args) = &either;
        fn.ret = &r;

        fn.run();
        ASSERT_EQ(r, result);
    }
}

struct ListInt_;
typedef Tuple<Int, ListInt_ *> Cons;
struct ListInt_ {
    using type = Variant<Cons, Tuple<>>;
    type value;
    // cppcheck-suppress noExplicitConstructor
    ListInt_(type value) : value(value) {}
};
using ListInt = ListInt_::type;

struct ListIntSum : ParametricFn<Int, ListInt> {
    void body() {
        ListInt list = *std::get<0>(args);
        switch (list.tag) {
        case 0: {
            Cons cons = *reinterpret_cast<Cons *>(&list.value);
            ListInt_ *tail = std::get<1>(cons);
            Int *head = new Int{std::get<0>(cons)};

            Int *r = new Int{};
            ListIntSum *tail_sum = new ListIntSum{};
            tail_sum->args = std::make_tuple(&tail->value);
            tail_sum->ret = r;

            Plus__BuiltIn *plus = new Plus__BuiltIn{};
            plus->ret = this->ret;
            plus->args = std::make_tuple(r, head);
            plus->deps = 1;

            tail_sum->conts = {plus};
            tail_sum->run();

            break;
        }
        case 1:
            *ret = 0;
            break;
        }
    }
};

GTEST_TEST(FnTests, RecursiveTypeTest) {
    ListInt tail{};
    tail.tag = 1;
    ListInt_ wrapped_tail = tail;
    ListInt third{};
    third.tag = 0;
    *reinterpret_cast<Cons *>(&third.value) = Cons(8, &wrapped_tail);
    ListInt_ wrapped_third = third;
    ListInt second{};
    second.tag = 0;
    *reinterpret_cast<Cons *>(&second.value) = Cons(4, &wrapped_third);
    ListInt_ wrapped_second = second;
    ListInt first{};
    first.tag = 0;
    *reinterpret_cast<Cons *>(&first.value) = Cons(-9, &wrapped_second);

    Int r = 0;
    ListIntSum adder{};
    adder.args = std::make_tuple(&first);
    adder.ret = &r;
    ASSERT_EQ(r, 0);

    adder.run();
    ASSERT_EQ(r, 3);
}
