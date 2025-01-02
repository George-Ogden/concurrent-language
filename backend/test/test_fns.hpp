#pragma once

#include "data_structures/lazy.hpp"
#include "fn/fn.hpp"
#include "fn/operators.hpp"
#include "system/work_manager.hpp"

#include <gtest/gtest.h>

#include <type_traits>
#include <utility>
#include <vector>

class FnCorrectnessTest : public ::testing::TestWithParam<unsigned> {
  protected:
    void SetUp() override {
        auto num_cpus = GetParam();
        ThreadManager::override_concurrency(num_cpus);
    }

    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

struct IdentityInt : ParametricFn<Int, Int> {
    using ParametricFn<Int, Int>::ParametricFn;
    Int body(Int &x) override { return x; }
};

TEST_P(FnCorrectnessTest, IdentityTest) {
    Int x = 5;
    IdentityInt *id = new IdentityInt{x};

    WorkManager::run(id);
    ASSERT_EQ(id->ret, 5);
}

struct FourWayPlusV1 : ParametricFn<Int, Int, Int, Int, Int> {
    using ParametricFn<Int, Int, Int, Int, Int>::ParametricFn;
    Plus__BuiltIn *call1 = nullptr, *call2 = nullptr, *call3 = nullptr;
    Int body(Int &a, Int &b, Int &c, Int &d) override {
        initialize(call1);
        call1->args = reference_all(a, b);
        call1->call();
        initialize(call2);
        call2->args = reference_all(c, d);
        call2->call();
        initialize(call3);
        call3->args = std::make_tuple(call1, call2);
        call3->run();
        return call3->value();
    }
};

struct FourWayPlusV2 : ParametricFn<Int, Int, Int, Int, Int> {
    using ParametricFn<Int, Int, Int, Int, Int>::ParametricFn;
    Plus__BuiltIn *call1 = nullptr, *call2 = nullptr, *call3 = nullptr;
    Int body(Int &a, Int &b, Int &c, Int &d) override {
        initialize(call1);
        call1->args = reference_all(a, b);
        call1->call();
        initialize(call2);
        call2->args = std::tuple_cat(std::make_tuple(call1), reference_all(c));
        call2->call();
        initialize(call3);
        call3->args = std::tuple_cat(std::make_tuple(call2), reference_all(d));
        call3->run();
        return call3->value();
    }
};

TEST_P(FnCorrectnessTest, FourWayPlusV1Test) {
    Int w = 11, x = 5, y = 10, z = 22;
    FourWayPlusV1 *plus = new FourWayPlusV1{w, x, y, z};

    WorkManager::run(plus);
    ASSERT_EQ(plus->ret, 48);
}

TEST_P(FnCorrectnessTest, FourWayPlusV2Test) {
    Int w = 11, x = 5, y = 10, z = 22;
    FourWayPlusV2 *plus = new FourWayPlusV2{w, x, y, z};

    WorkManager::run(plus);
    ASSERT_EQ(plus->ret, 48);
}

struct BranchingExample : ParametricFn<Int, Int, Int, Int> {
    using ParametricFn<Int, Int, Int, Int>::ParametricFn;
    Comparison_GE__BuiltIn *call1 = nullptr;
    Plus__BuiltIn *call2 = nullptr;
    Minus__BuiltIn *call3 = nullptr;
    Int body(Int &x, Int &y, Int &z) override {
        initialize(call1);
        call1->args = reference_all(x, Int(0));
        call1->run();
        if (call1->value()) {
            initialize(call2);
            call2->args = reference_all(y, Int(1));
            call2->call();
        } else {
            initialize(call2);
            call2->args = reference_all(z, Int(1));
            call2->call();
        }
        initialize(call3);
        call3->args =
            std::tuple_cat(std::make_tuple(call2), reference_all(Int(2)));
        call3->run();
        return call3->value();
    }
};

TEST_P(FnCorrectnessTest, PositiveBranchingExampleTest) {
    Int x = 5, y = 10, z = 22;
    BranchingExample *branching = new BranchingExample{x, y, z};

    WorkManager::run(branching);
    ASSERT_EQ(branching->ret, 9);
}

TEST_P(FnCorrectnessTest, NegativeBranchingExampleTest) {
    Int x = -5, y = 10, z = 22;
    BranchingExample *branching = new BranchingExample{x, y, z};

    WorkManager::run(branching);
    ASSERT_EQ(branching->ret, 21);
}

struct FlatBlockExample : ParametricFn<Int, Int> {
    using ParametricFn<Int, Int>::ParametricFn;
    Increment__BuiltIn *call1 = nullptr, *call2 = nullptr, *call3;
    ParametricFn<Int> *block1 = nullptr, *block2 = nullptr, *block3 = nullptr;
    Int body(Int &x) override {
        if (block1 == nullptr) {
            block1 = new BlockFn<Int>([&]() {
                initialize(call1);
                call1->args = reference_all(x);
                call1->run();
                return call1->value();
            });
        }
        block1->run();
        return block1->value();
    }
};

TEST_P(FnCorrectnessTest, FlatBlockExampleTest) {
    Int x = 5;
    FlatBlockExample *block = new FlatBlockExample{x};

    WorkManager::run(block);
    ASSERT_EQ(block->ret, 6);
}

struct NestedBlockExample : ParametricFn<Int, Int> {
    using ParametricFn<Int, Int>::ParametricFn;
    Increment__BuiltIn *call1 = nullptr, *call2 = nullptr, *call3 = nullptr;
    ParametricFn<Int> *block1 = nullptr, *block2 = nullptr, *block3 = nullptr;
    Int body(Int &x) override {
        if (block1 == nullptr) {
            block1 = new BlockFn<Int>([&]() {
                initialize(call1);
                call1->args = reference_all(x);
                call1->call();
                if (block2 == nullptr) {
                    block2 = new BlockFn<Int>([&]() {
                        initialize(call2);
                        call2->args = std::make_tuple(call1);
                        call2->call();
                        if (block3 == nullptr) {
                            block3 = new BlockFn<Int>([&] {
                                initialize(call3);
                                call3->args = std::make_tuple(call2);
                                call3->run();
                                return call3->value();
                            });
                        }
                        block3->run();
                        return block3->value();
                    });
                }
                block2->run();
                return block2->value();
            });
        }
        block1->run();
        return block1->value();
    }
};

TEST_P(FnCorrectnessTest, NestedBlockExampleTest) {
    Int x = 5;
    NestedBlockExample *block = new NestedBlockExample{x};

    WorkManager::run(block);
    ASSERT_EQ(block->ret, 8);
}

struct IfStatementExample : ParametricFn<Int, Int, Int, Int> {
    using ParametricFn<Int, Int, Int, Int>::ParametricFn;
    Comparison_GE__BuiltIn *call1 = nullptr;
    ParametricFn<Int> *branch = nullptr;
    Plus__BuiltIn *call2_1 = nullptr, *call2_2 = nullptr;
    Minus__BuiltIn *call3 = nullptr;
    Int body(Int &x, Int &y, Int &z) override {
        Int a = 1;
        auto branch1 = Block([&]() {
            initialize(call2_1);
            call2_1->args = reference_all(y, a);
            call2_1->run();
            return call2_1->value();
        });
        auto branch2 = Block([&]() {
            initialize(call2_2);
            call2_2->args = reference_all(z, a);
            call2_2->run();
            return call2_2->value();
        });

        initialize(call1);
        call1->args = reference_all(x, Int(0));
        call1->run();
        if (call1->value()) {
            if (branch == nullptr) {
                branch = &branch1;
            }
            branch->call();
        } else {
            if (branch == nullptr) {
                branch = &branch2;
            };
            branch->call();
        }
        initialize(call3);
        call3->args =
            std::tuple_cat(std::make_tuple(branch), reference_all(Int(2)));
        call3->run();
        return call3->value();
    }
};

TEST_P(FnCorrectnessTest, IfStatementExampleTest) {
    Int x = 5, y = 10, z = 22;
    IfStatementExample *branching = new IfStatementExample{x, y, z};

    WorkManager::run(branching);
    ASSERT_EQ(branching->ret, 9);
}

struct RecursiveDouble : ParametricFn<Int, Int> {
    using ParametricFn<Int, Int>::ParametricFn;
    RecursiveDouble *call1 = nullptr, *call3 = nullptr;
    Plus__BuiltIn *call2 = nullptr;
    Int body(Int &x) override {
        if (x > 0) {
            initialize(call1);
            call1->args = reference_all(x - 1);
            call1->call();

            initialize(call3);
            call3->args = reference_all(x - 1);
            call3->run();

            initialize(call2);
            call2->args =
                std::tuple_cat(std::make_tuple(call1), reference_all(Int(2)));
            call2->run();
            return call2->value();
        } else {
            return 0;
        }
    }
};

TEST_P(FnCorrectnessTest, RecursiveDoubleTest1) {
    Int x = 2;
    RecursiveDouble *double_ = new RecursiveDouble{x};

    WorkManager::run(double_);
    ASSERT_EQ(double_->ret, 4);
}

TEST_P(FnCorrectnessTest, RecursiveDoubleTest2) {
    Int x = -5;
    RecursiveDouble *double_ = new RecursiveDouble{x};

    WorkManager::run(double_);
    ASSERT_EQ(double_->ret, 0);
}

TEST_P(FnCorrectnessTest, RecursiveDoubleTest3) {
    Int x = 8;
    RecursiveDouble *double_ = new RecursiveDouble{x};

    WorkManager::run(double_);
    ASSERT_EQ(double_->ret, 16);
}

struct EvenOrOdd : ParametricFn<Bool, Int> {
    Bool body(Int &x) override { return static_cast<bool>(x); }
};

struct ApplyIntBool : ParametricFn<Bool, ParametricFn<Bool, Int> *, Int> {
    using ParametricFn<Bool, ParametricFn<Bool, Int> *, Int>::ParametricFn;
    Bool body(ParametricFn<Bool, Int> *&f, Int &x) override {
        f->args = reference_all(x);
        f->run();
        return f->value();
    }
};

TEST_P(FnCorrectnessTest, HigherOrderFunctionTest) {
    ParametricFn<Bool, Int> *f = new EvenOrOdd{};
    Int x = 5;
    ApplyIntBool *apply = new ApplyIntBool{f, x};

    WorkManager::run(apply);
    ASSERT_TRUE(apply->ret);
}

struct PairIntBool : ParametricFn<std::tuple<Int, Bool>, Int, Bool> {
    using ParametricFn<std::tuple<Int, Bool>, Int, Bool>::ParametricFn;
    std::tuple<Int, Bool> body(Int &x, Bool &y) override {
        return std::make_tuple(x, y);
    }
};

TEST_P(FnCorrectnessTest, TupleTest) {
    Int x = 5;
    Bool y = true;
    PairIntBool *pair = new PairIntBool{x, y};

    WorkManager::run(pair);
    ASSERT_EQ(pair->ret, std::make_tuple(5, true));
}

using Bull = Variant<std::monostate, std::monostate>;

struct BoolUnion : ParametricFn<Bool, Bull> {
    using ParametricFn<Bool, Bull>::ParametricFn;
    bool body(Bull &x) override { return x.tag == 0; }
};

TEST_P(FnCorrectnessTest, ValueFreeUnionTest) {
    {
        Bull bull{};
        bull.tag = 0;
        BoolUnion *fn = new BoolUnion{bull};

        WorkManager::run(fn);
        ASSERT_TRUE(fn->ret);
    }

    {
        Bull bull{};
        bull.tag = 1;
        BoolUnion *fn = new BoolUnion{bull};

        WorkManager::run(fn);
        ASSERT_FALSE(fn->ret);
    }
}

using EitherIntBool = Variant<Int, Bool>;

struct EitherIntBoolExtractor : ParametricFn<Bool, EitherIntBool> {
    using ParametricFn<Bool, EitherIntBool>::ParametricFn;
    Bool body(EitherIntBool &x) override {
        switch (x.tag) {
        case 0:
            return *reinterpret_cast<int *>(&x.value) > 10;
        case 1:
            return *reinterpret_cast<bool *>(&x.value);
        }
        return 0;
    }
};

TEST_P(FnCorrectnessTest, ValueIncludedUnionTest) {
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

        EitherIntBoolExtractor *fn = new EitherIntBoolExtractor{either};

        WorkManager::run(fn);
        ASSERT_EQ(fn->ret, result);
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
    using ParametricFn<Int, ListInt>::ParametricFn;
    ListIntSum *call1 = nullptr;
    Plus__BuiltIn *call2 = nullptr;
    Int body(ListInt &list) override {
        switch (list.tag) {
        case 0: {
            Cons cons = *reinterpret_cast<Cons *>(&list.value);
            ListInt_ *tail = std::get<1>(cons);
            Int head = std::get<0>(cons);

            initialize(call1);
            call1->args = reference_all(tail->value);
            call1->call();

            initialize(call2);
            call2->args =
                std::tuple_cat(std::make_tuple(call1), reference_all(head));
            call2->run();
            return call2->value();
        }
        case 1:
            return 0;
        }
        return 0;
    }
};

TEST_P(FnCorrectnessTest, RecursiveTypeTest) {
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

    ListIntSum *adder = new ListIntSum{first};

    WorkManager::run(adder);
    ASSERT_EQ(adder->ret, 3);
}

const std::vector<unsigned> cpu_counts = {1, 2, 3, 4};
INSTANTIATE_TEST_SUITE_P(FnCorrectnessTests, FnCorrectnessTest,
                         ::testing::ValuesIn(cpu_counts));
