#pragma once

#include "data_structures/lazy.hpp"
#include "fn/fn.hpp"
#include "fn/operators.hpp"
#include "system/work_manager.hpp"
#include "types/builtin.hpp"
#include "types/compound.hpp"

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

struct IdentityInt : EasyCloneFn<IdentityInt, Int, Int> {
    using EasyCloneFn<IdentityInt, Int, Int>::EasyCloneFn;
    Lazy<Int> *body(Lazy<Int> *&x) override { return x; }
};

TEST_P(FnCorrectnessTest, IdentityTest) {
    Int x = 5;
    IdentityInt *id = new IdentityInt{x};

    WorkManager::run(id);
    ASSERT_EQ(id->ret, 5);
}

struct FourWayPlusV1 : EasyCloneFn<FourWayPlusV1, Int, Int, Int, Int, Int> {
    using EasyCloneFn<FourWayPlusV1, Int, Int, Int, Int, Int>::EasyCloneFn;
    Plus__BuiltIn *call1 = nullptr, *call2 = nullptr, *call3 = nullptr;
    Lazy<Int> *body(Lazy<Int> *&a, Lazy<Int> *&b, Lazy<Int> *&c,
                    Lazy<Int> *&d) override {
        if (call1 == nullptr) {
            call1 = new Plus__BuiltIn(a, b);
            call1->call();
        }
        if (call2 == nullptr) {
            call2 = new Plus__BuiltIn(c, d);
            call2->call();
        }
        if (call3 == nullptr) {
            call3 = new Plus__BuiltIn(call1, call2);
            call3->call();
        }
        return call3;
    }
};

struct FourWayPlusV2 : EasyCloneFn<FourWayPlusV2, Int, Int, Int, Int, Int> {
    using EasyCloneFn<FourWayPlusV2, Int, Int, Int, Int, Int>::EasyCloneFn;
    Plus__BuiltIn *call1 = nullptr, *call2 = nullptr, *call3 = nullptr;
    Lazy<Int> *body(Lazy<Int> *&a, Lazy<Int> *&b, Lazy<Int> *&c,
                    Lazy<Int> *&d) override {
        if (call1 == nullptr) {
            call1 = new Plus__BuiltIn(a, b);
            call1->call();
        }
        if (call2 == nullptr) {
            call2 = new Plus__BuiltIn(call1, c);
            call2->call();
        }
        if (call3 == nullptr) {
            call3 = new Plus__BuiltIn(call2, d);
            call3->call();
        }
        return call3;
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

struct BranchingExample : EasyCloneFn<BranchingExample, Int, Int, Int, Int> {
    using EasyCloneFn<BranchingExample, Int, Int, Int, Int>::EasyCloneFn;
    Comparison_GE__BuiltIn *call1 = nullptr;
    Plus__BuiltIn *call2 = nullptr;
    Minus__BuiltIn *call3 = nullptr;
    Lazy<Int> *body(Lazy<Int> *&x, Lazy<Int> *&y, Lazy<Int> *&z) override {
        if (call1 == nullptr) {
            call1 = new Comparison_GE__BuiltIn{x, new LazyConstant<Int>(0)};
            call1->call();
        }
        WorkManager::await(call1);
        call1->run();
        if (call1->value()) {
            if (call2 == nullptr) {
                call2 = new Plus__BuiltIn{y, new LazyConstant<Int>(1)};
                call2->call();
            }
        } else {
            if (call2 == nullptr) {
                call2 = new Plus__BuiltIn{z, new LazyConstant<Int>(1)};
                call2->call();
            }
        }
        if (call3 == nullptr) {
            call3 = new Minus__BuiltIn{call2, new LazyConstant<Int>(2)};
            call3->call();
        }
        return call3;
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

struct FlatBlockExample : EasyCloneFn<FlatBlockExample, Int, Int> {
    using EasyCloneFn<FlatBlockExample, Int, Int>::EasyCloneFn;
    Increment__BuiltIn *call1 = nullptr;
    ParametricFn<Int> *block1 = nullptr;
    Lazy<Int> *body(Lazy<Int> *&x) override {
        if (block1 == nullptr) {
            block1 = new BlockFn<Int>([&]() {
                if (call1 == nullptr) {
                    call1 = new Increment__BuiltIn{x};
                    call1->call();
                }
                return call1;
            });
            block1->call();
        }
        return block1;
    }
};

TEST_P(FnCorrectnessTest, FlatBlockExampleTest) {
    Int x = 5;
    FlatBlockExample *block = new FlatBlockExample{x};

    WorkManager::run(block);
    ASSERT_EQ(block->ret, 6);
}

struct NestedBlockExample : EasyCloneFn<NestedBlockExample, Int, Int> {
    using EasyCloneFn<NestedBlockExample, Int, Int>::EasyCloneFn;
    Increment__BuiltIn *call1 = nullptr, *call2 = nullptr, *call3 = nullptr;
    ParametricFn<Int> *block1 = nullptr, *block2 = nullptr, *block3 = nullptr;
    Lazy<Int> *body(Lazy<Int> *&x) override {
        if (block1 == nullptr) {
            block1 = new BlockFn<Int>([&]() {
                if (call1 == nullptr) {
                    call1 = new Increment__BuiltIn{x};
                    call1->call();
                }
                if (block2 == nullptr) {
                    block2 = new BlockFn<Int>([&]() {
                        if (call2 == nullptr) {
                            call2 = new Increment__BuiltIn{call1};
                            call2->call();
                        }
                        if (block3 == nullptr) {
                            block3 = new BlockFn<Int>([&] {
                                if (call3 == nullptr) {
                                    call3 = new Increment__BuiltIn{call2};
                                    call3->call();
                                }
                                return call3;
                            });
                            block3->call();
                        }
                        return block3;
                    });
                    block2->call();
                }
                return block2;
            });
            block1->call();
        }
        return block1;
    }
};

TEST_P(FnCorrectnessTest, NestedBlockExampleTest) {
    Int x = 5;
    NestedBlockExample *block = new NestedBlockExample{x};

    WorkManager::run(block);
    ASSERT_EQ(block->ret, 8);
}

struct IfStatementExample
    : EasyCloneFn<IfStatementExample, Int, Int, Int, Int> {
    using EasyCloneFn<IfStatementExample, Int, Int, Int, Int>::EasyCloneFn;
    Comparison_GE__BuiltIn *call1 = nullptr;
    ParametricFn<Int> *branch1 = nullptr, *branch2 = nullptr, *branch = nullptr;
    Plus__BuiltIn *call2_1 = nullptr, *call2_2 = nullptr;
    Minus__BuiltIn *call3 = nullptr;
    Lazy<Int> *body(Lazy<Int> *&x, Lazy<Int> *&y, Lazy<Int> *&z) override {
        if (branch1 == nullptr) {
            branch1 = new BlockFn<Int>([&]() {
                if (call2_1 == nullptr) {
                    call2_1 = new Plus__BuiltIn{y, new LazyConstant<Int>(1)};
                    call2_1->call();
                }
                return call2_1;
            });
        }
        if (branch2 == nullptr) {
            branch2 = new BlockFn<Int>([&]() {
                if (call2_2 == nullptr) {
                    call2_2 = new Plus__BuiltIn{z, new LazyConstant<Int>(1)};
                    call2_2->call();
                }
                return call2_2;
            });
        }

        if (call1 == nullptr) {
            call1 = new Comparison_GE__BuiltIn{x, new LazyConstant<Int>(0)};
            call1->call();
        }
        WorkManager::await(call1);
        if (call1->value()) {
            if (branch == nullptr) {
                branch = branch1;
                branch->call();
            }
        } else {
            if (branch == nullptr) {
                branch = branch2;
                branch->call();
            };
        }
        if (call3 == nullptr) {
            call3 = new Minus__BuiltIn{branch, new LazyConstant<Int>(2)};
            call3->call();
        }
        return call3;
    }
};

TEST_P(FnCorrectnessTest, IfStatementExampleTest) {
    Int x = 5, y = 10, z = 22;
    IfStatementExample *branching = new IfStatementExample{x, y, z};

    WorkManager::run(branching);
    ASSERT_EQ(branching->ret, 9);
}

struct RecursiveDouble : EasyCloneFn<RecursiveDouble, Int, Int> {
    using EasyCloneFn<RecursiveDouble, Int, Int>::EasyCloneFn;
    RecursiveDouble *call1 = nullptr, *call3 = nullptr;
    Plus__BuiltIn *call2 = nullptr;
    Lazy<Int> *body(Lazy<Int> *&x) override {
        WorkManager::await(x);
        if (x->value() > 0) {
            if (call1 == nullptr) {
                auto arg = new Decrement__BuiltIn{x};
                arg->call();
                call1 = new RecursiveDouble{arg};
                call1->call();
            }

            if (call3 == nullptr) {
                auto arg = new Decrement__BuiltIn{x};
                arg->call();
                call3 = new RecursiveDouble{arg};
                call3->run();
            }

            if (call2 == nullptr) {
                call2 = new Plus__BuiltIn{call1, new LazyConstant<Int>(2)};
                call2->call();
            }
            return call2;
        } else {
            return new LazyConstant<Int>(0);
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

struct EvenOrOdd : EasyCloneFn<EvenOrOdd, Bool, Int> {
    using EasyCloneFn<EvenOrOdd, Bool, Int>::EasyCloneFn;
    Lazy<Bool> *body(Lazy<Int> *&x) override {
        WorkManager::await(x);
        return new LazyConstant<Bool>(x->value() & 1);
    }
};

struct ApplyIntBool : EasyCloneFn<ApplyIntBool, Bool, FnT<Bool, Int>, Int> {
    using EasyCloneFn<ApplyIntBool, Bool, FnT<Bool, Int>, Int>::EasyCloneFn;
    Lazy<Bool> *body(Lazy<FnT<Bool, Int>> *&f, Lazy<Int> *&x) override {
        WorkManager::await(f);
        auto g = f->value();
        g->args = std::make_tuple(x);
        g->call();
        return g;
    }
};

TEST_P(FnCorrectnessTest, HigherOrderFunctionTest) {
    FnT<Bool, Int> f = new EvenOrOdd{};
    Int x = 5;
    ApplyIntBool *apply = new ApplyIntBool{f, x};

    WorkManager::run(apply);
    ASSERT_TRUE(apply->ret);
}

struct PairIntBool : EasyCloneFn<PairIntBool, TupleT<Int, Bool>, Int, Bool> {
    using EasyCloneFn<PairIntBool, TupleT<Int, Bool>, Int, Bool>::EasyCloneFn;
    Lazy<TupleT<Int, Bool>> *body(Lazy<Int> *&x, Lazy<Bool> *&y) override {
        WorkManager::await(x, y);
        return new LazyConstant<TupleT<Int, Bool>>(
            std::make_tuple(x->value(), y->value()));
    }
};

struct HigherOrderReuse
    : EasyCloneFn<HigherOrderReuse, Int, FnT<Int, Int>, Int, Int> {
    using EasyCloneFn<HigherOrderReuse, Int, FnT<Int, Int>, Int,
                      Int>::EasyCloneFn;
    FnT<Int, Int> call1 = nullptr, call2 = nullptr;
    Plus__BuiltIn *call3 = nullptr;
    Lazy<Int> *body(Lazy<FnT<Int, Int>> *&f, Lazy<Int> *&x,
                    Lazy<Int> *&y) override {
        WorkManager::await(f);
        if (call1 == nullptr) {
            call1 = f->value();
            call1->args = std::make_tuple(x);
            call1->call();
        }
        if (call2 == nullptr) {
            call2 = call1->clone();
            call2->args = std::make_tuple(y);
            call2->call();
        }
        if (call3 == nullptr) {
            call3 = new Plus__BuiltIn{call1, call2};
            call3->call();
        }
        return call3;
    }
};

TEST_P(FnCorrectnessTest, ReusedHigherOrderFunctionTest) {
    FnT<Int, Int> f = new Increment__BuiltIn{};
    Int x = 5, y = 4;
    HigherOrderReuse *F = new HigherOrderReuse{f, x, y};

    WorkManager::run(F);
    ASSERT_EQ(F->ret, 11);
}

TEST_P(FnCorrectnessTest, TupleTest) {
    Int x = 5;
    Bool y = true;
    PairIntBool *pair = new PairIntBool{x, y};

    WorkManager::run(pair);
    ASSERT_EQ(pair->ret, std::make_tuple(5, true));
}

struct Twoo;
struct Faws;
typedef VariantT<Twoo, Faws> Bull;
struct Twoo {};
struct Faws {};

struct BoolUnion : EasyCloneFn<BoolUnion, Bool, Bull> {
    using EasyCloneFn<BoolUnion, Bool, Bull>::EasyCloneFn;
    Lazy<Bool> *body(Lazy<Bull> *&x) override {
        WorkManager::await(x);
        return new LazyConstant<Bool>(x->value().tag == 0);
    }
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

struct Left;
struct Right;
typedef VariantT<Left, Right> EitherIntBool;
struct Left {
    Int value;
};
struct Right {
    Bool value;
};

struct EitherIntBoolExtractor
    : EasyCloneFn<EitherIntBoolExtractor, Bool, EitherIntBool> {
    using EasyCloneFn<EitherIntBoolExtractor, Bool, EitherIntBool>::EasyCloneFn;
    Lazy<Bool> *body(Lazy<EitherIntBool> *&either) override {
        WorkManager::await(either);
        EitherIntBool x = either->value();
        switch (x.tag) {
        case 0ULL:
            return new LazyConstant<Bool>(
                reinterpret_cast<Left *>(&x.value)->value > 10);
        case 1ULL:
            return new LazyConstant<Bool>(
                reinterpret_cast<Right *>(&x.value)->value);
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

            *reinterpret_cast<Int *>(&either.value) = value;
        } else {
            *reinterpret_cast<Bool *>(&either.value) = value;
        }

        EitherIntBoolExtractor *fn = new EitherIntBoolExtractor{either};

        WorkManager::run(fn);
        ASSERT_EQ(fn->ret, result);
    }
}

struct Cons;
struct Nil;
typedef VariantT<Cons, Nil> ListInt;
struct Cons {
    using type = TupleT<Int, ListInt *>;
    type value;
};
struct Nil {};

struct ListIntSum : EasyCloneFn<ListIntSum, Int, ListInt> {
    using EasyCloneFn<ListIntSum, Int, ListInt>::EasyCloneFn;
    ListIntSum *call1 = nullptr;
    Plus__BuiltIn *call2 = nullptr;
    Lazy<Int> *body(Lazy<ListInt> *&lazy_list) override {
        WorkManager::await(lazy_list);
        ListInt list = lazy_list->value();
        switch (list.tag) {
        case 0: {
            Cons::type cons = reinterpret_cast<Cons *>(&list.value)->value;
            Int head = std::get<0ULL>(cons);
            ListInt *tail_ = std::get<1ULL>(cons);
            ListInt tail = *tail_;

            if (call1 == nullptr) {
                call1 = new ListIntSum{};
                call1->args = reference_all(tail);
                call1->call();
            }

            if (call2 == nullptr) {
                call2 = new Plus__BuiltIn{};
                call2->args =
                    std::tuple_cat(std::make_tuple(call1), reference_all(head));
                call2->call();
            }
            return call2;
        }
        case 1:
            return new LazyConstant<Int>(0);
        }
        return nullptr;
    }
};

TEST_P(FnCorrectnessTest, RecursiveTypeTest) {
    ListInt tail{};
    tail.tag = 1;
    ListInt wrapped_tail = tail;
    ListInt third{};
    third.tag = 0;
    *reinterpret_cast<Cons *>(&third.value) =
        Cons{std::make_tuple(8, &wrapped_tail)};
    ListInt wrapped_third = third;
    ListInt second{};
    second.tag = 0;
    *reinterpret_cast<Cons *>(&second.value) =
        Cons{std::make_tuple(4, &wrapped_third)};
    ListInt wrapped_second = second;
    ListInt first{};
    first.tag = 0;
    *reinterpret_cast<Cons *>(&first.value) =
        Cons{std::make_tuple(-9, &wrapped_second)};

    ListIntSum *adder = new ListIntSum{first};

    WorkManager::run(adder);
    ASSERT_EQ(adder->ret, 3);
}

struct Suc;
typedef VariantT<Suc, Nil> Nat;
struct Suc {
    using type = Nat *;
    type value;
};

struct SimpleRecursiveTypeExample
    : EasyCloneFn<SimpleRecursiveTypeExample, VariantT<Suc, Nil>,
                  VariantT<Suc, Nil>> {
    using EasyCloneFn<SimpleRecursiveTypeExample, VariantT<Suc, Nil>,
                      VariantT<Suc, Nil>>::EasyCloneFn;
    Lazy<VariantT<Suc, Nil>> *body(Lazy<VariantT<Suc, Nil>> *&nat_) override {
        WorkManager::await(nat_);
        VariantT<Suc, Nil> nat = nat_->value();
        switch (nat.tag) {
        case 0: {
            Suc::type s = reinterpret_cast<Suc *>(&nat.value)->value;
            VariantT<Suc, Nil> r = *s;
            return new LazyConstant<VariantT<Suc, Nil>>{r};
        }
        case 1: {
            VariantT<Suc, Nil> n{};
            n.tag = 1;
            return new LazyConstant<VariantT<Suc, Nil>>{n};
        }
        }
        return nullptr;
    }
};

TEST_P(FnCorrectnessTest, SimpleRecursiveTypeTest) {
    VariantT<Suc, Nil> n{};
    n.tag = 1;
    Nat *wrapped_n = new Nat{n};

    VariantT<Suc, Nil> inner{};
    inner.tag = 0;
    *reinterpret_cast<Suc *>(&inner.value) = Suc{wrapped_n};
    Nat *wrapped_inner = new Nat{inner};

    VariantT<Suc, Nil> outer{};
    outer.tag = 0;
    *reinterpret_cast<Suc *>(&outer.value) = Suc{wrapped_inner};

    SimpleRecursiveTypeExample *fn = new SimpleRecursiveTypeExample{outer};

    WorkManager::run(fn);
    ASSERT_EQ(fn->ret.tag, inner.tag);
    ASSERT_EQ(reinterpret_cast<Suc *>(&fn->ret.value)->value,
              reinterpret_cast<Suc *>(&inner.value)->value);
}

const std::vector<unsigned> cpu_counts = {1, 2, 3, 4};
INSTANTIATE_TEST_SUITE_P(FnCorrectnessTests, FnCorrectnessTest,
                         ::testing::ValuesIn(cpu_counts));
