#pragma once

#include "data_structures/lazy.hpp"
#include "fn/fn.hpp"
#include "fn/operators.hpp"
#include "system/work_manager.hpp"
#include "types/builtin.hpp"
#include "types/compound.hpp"
#include "types/utils.hpp"

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

struct IdentityInt : Closure<IdentityInt, Empty, Int, Int> {
    using Closure<IdentityInt, Empty, Int, Int>::Closure;
    Lazy<Int> *body(Lazy<Int> *&x) override { return x; }
};

TEST_P(FnCorrectnessTest, IdentityTest) {
    Int x = 5;
    IdentityInt *id = new IdentityInt{};
    id->args = std::make_tuple(new LazyConstant<Int>{x});

    WorkManager::run(id);
    ASSERT_EQ(id->ret, 5);
}

struct FourWayPlusV1 : Closure<FourWayPlusV1, Empty, Int, Int, Int, Int, Int> {
    using Closure<FourWayPlusV1, Empty, Int, Int, Int, Int, Int>::Closure;
    FnT<Int, Int, Int> call1 = nullptr, call2 = nullptr, call3 = nullptr;
    Lazy<Int> *body(Lazy<Int> *&a, Lazy<Int> *&b, Lazy<Int> *&c,
                    Lazy<Int> *&d) override {
        if (call1 == nullptr) {
            call1 = new Plus__BuiltIn{};
            call1->args = std::make_tuple(a, b);
            call1->call();
        }
        if (call2 == nullptr) {
            call2 = new Plus__BuiltIn{};
            call2->args = std::make_tuple(c, d);
            call2->call();
        }
        if (call3 == nullptr) {
            call3 = new Plus__BuiltIn{};
            call3->args = std::make_tuple(call1, call2);
            call3->call();
        }
        return call3;
    }
};

struct FourWayPlusV2 : Closure<FourWayPlusV2, Empty, Int, Int, Int, Int, Int> {
    using Closure<FourWayPlusV2, Empty, Int, Int, Int, Int, Int>::Closure;
    Plus__BuiltIn *call1 = nullptr, *call2 = nullptr, *call3 = nullptr;
    Lazy<Int> *body(Lazy<Int> *&a, Lazy<Int> *&b, Lazy<Int> *&c,
                    Lazy<Int> *&d) override {
        if (call1 == nullptr) {
            call1 = new Plus__BuiltIn{a, b};
            call1->call();
        }
        if (call2 == nullptr) {
            call2 = new Plus__BuiltIn{call1, c};
            call2->call();
        }
        if (call3 == nullptr) {
            call3 = new Plus__BuiltIn{call2, d};
            call3->call();
        }
        return call3;
    }
};

TEST_P(FnCorrectnessTest, FourWayPlusV1Test) {
    Int w = 11, x = 5, y = 10, z = 22;
    FourWayPlusV1 *plus = new FourWayPlusV1{};
    plus->args =
        std::make_tuple(new LazyConstant<Int>{w}, new LazyConstant<Int>{x},
                        new LazyConstant<Int>{y}, new LazyConstant<Int>{z});

    WorkManager::run(plus);
    ASSERT_EQ(plus->ret, 48);
}

TEST_P(FnCorrectnessTest, FourWayPlusV2Test) {
    Int w = 11, x = 5, y = 10, z = 22;
    FourWayPlusV2 *plus = new FourWayPlusV2{};
    plus->args =
        std::make_tuple(new LazyConstant<Int>{w}, new LazyConstant<Int>{x},
                        new LazyConstant<Int>{y}, new LazyConstant<Int>{z});

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
    FnT<Int, Int> call1 = nullptr;
    FnT<Int> block1 = nullptr;
    Lazy<Int> *body(Lazy<Int> *&x) override {
        if (block1 == nullptr) {
            block1 = new BlockFn<Int>([&]() {
                if (call1 == nullptr) {
                    call1 = new Increment__BuiltIn{};
                    call1->args = std::make_tuple(x);
                    call1->call();
                }
                return call1;
            });
            block1->call();
        }
        block1->args = std::make_tuple();
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

struct Adder : Closure<Adder, Lazy<Int> *, Int, Int> {
    using Closure<Adder, Lazy<Int> *, Int, Int>::Closure;
    FnT<Int, Int, Int> inner_res = nullptr;
    Lazy<Int> *body(Lazy<Int> *&x) override {
        if (inner_res == nullptr) {
            inner_res = new Plus__BuiltIn{x, env};
            inner_res->call();
        }
        return inner_res;
    }
};

struct NestedFnExample : EasyCloneFn<NestedFnExample, Int, Int> {
    using EasyCloneFn<NestedFnExample, Int, Int>::EasyCloneFn;
    FnT<Int, Int> closure = nullptr;
    FnT<Int, Int> res = nullptr;
    Lazy<Int> *body(Lazy<Int> *&x) override {
        if (closure == nullptr) {
            closure = new Adder{x};
        }
        if (res == nullptr) {
            res = closure->clone();
            res->args = std::make_tuple(x);
            res->call();
        }
        return res;
    }
};

TEST_P(FnCorrectnessTest, NestedFnExampleTest) {
    Int x = 5;
    NestedFnExample *nested = new NestedFnExample{x};

    WorkManager::run(nested);
    ASSERT_EQ(nested->ret, 10);
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

struct SharedRegisterExample : EasyCloneFn<SharedRegisterExample, Int, Bool> {
    using EasyCloneFn<SharedRegisterExample, Int, Bool>::EasyCloneFn;
    Lazy<Int> *body(Lazy<Bool> *&b) override {
        WorkManager::await(b);
        Bool m0;
        Int m1;
        m0 = b->value();
        if (m0) {
            m1 = 1;
        } else {
            m1 = 0;
        }
        Lazy<Int> *m2 = new LazyConstant<Int>{m1};
        return m2;
    }
};

TEST_P(FnCorrectnessTest, SharedRegisterExampleTest) {
    Bool b = true;
    SharedRegisterExample *example = new SharedRegisterExample{b};

    WorkManager::run(example);
    ASSERT_EQ(example->ret, 1);
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
        bull.tag = 1ULL;
        BoolUnion *fn = new BoolUnion{bull};

        WorkManager::run(fn);
        ASSERT_FALSE(fn->ret);
    }
}

struct Left;
struct Right;
typedef VariantT<Left, Right> EitherIntBool;
struct Left {
    using type = Int;
    type value;
};
struct Right {
    using type = Bool;
    type value;
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

            reinterpret_cast<Left *>(&either.value)->value = value;
        } else {
            reinterpret_cast<Right *>(&either.value)->value = value;
        }

        EitherIntBoolExtractor *fn = new EitherIntBoolExtractor{either};

        WorkManager::run(fn);
        ASSERT_EQ(fn->ret, result);
    }
}

struct EitherIntBoolEdgeCase
    : EasyCloneFn<EitherIntBoolEdgeCase, Bool, EitherIntBool> {
    using EasyCloneFn<EitherIntBoolEdgeCase, Bool, EitherIntBool>::EasyCloneFn;
    Lazy<Bool> *y = nullptr;
    Lazy<Bool> *body(Lazy<EitherIntBool> *&either) override {
        WorkManager::await(either);
        EitherIntBool x = either->value();
        switch (x.tag) {
        case 0ULL: {
            Lazy<Left::type> *i = new LazyConstant<Left::type>{
                reinterpret_cast<Left *>(&x.value)->value};
            Lazy<Int> *z = new LazyConstant<Int>{0};
            if (y == nullptr) {
                y = new Comparison_GT__BuiltIn{};
                dynamic_cast<FnT<Bool, Int, Int>>(y)->args =
                    std::make_tuple(i, z);
                dynamic_cast<Fn *>(y)->call();
            }
            break;
        }
        case 1ULL: {
            Lazy<Right::type> *b = new LazyConstant<Right::type>{
                reinterpret_cast<Right *>(&x.value)->value};
            y = b;
            break;
        }
        }
        return y;
    }
};

TEST_P(FnCorrectnessTest, EdgeCaseTest) {
    for (const auto &[tag, value, result] :
         std::vector<std::tuple<int, int, bool>>{{1, 0, false},
                                                 {1, 1, true},
                                                 {0, 0, false},
                                                 {0, -5, false},
                                                 {0, 15, true}}) {
        EitherIntBool either{};
        either.tag = tag;
        if (tag == 0) {

            reinterpret_cast<Left *>(&either.value)->value = value;
        } else {
            reinterpret_cast<Right *>(&either.value)->value = value;
        }

        EitherIntBoolEdgeCase *fn = new EitherIntBoolEdgeCase{either};

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
            Lazy<destroy_references_t<Cons::type>> *cons_lazy =
                new LazyConstant<destroy_references_t<Cons::type>>{
                    destroy_references(
                        reinterpret_cast<Cons *>(&list.value)->value)};
            WorkManager::await(cons_lazy);
            destroy_references_t<Cons::type> cons = cons_lazy->value();
            Int head = std::get<0ULL>(cons);
            ListInt tail = std::get<1ULL>(cons);

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
    tail.tag = 1ULL;
    ListInt third{};
    third.tag = 0ULL;
    reinterpret_cast<Cons *>(&third.value)->value =
        create_references<Cons::type>(std::make_tuple(8, tail));
    ListInt second{};
    second.tag = 0ULL;
    reinterpret_cast<Cons *>(&second.value)->value =
        create_references<Cons::type>(std::make_tuple(4, third));
    ListInt first{};
    first.tag = 0ULL;
    reinterpret_cast<Cons *>(&first.value)->value =
        create_references<Cons::type>(std::make_tuple(-9, second));

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
            VariantT<Suc, Nil> n = {};
            n.tag = 1ULL;
            return new LazyConstant<VariantT<Suc, Nil>>{n};
        }
        }
        return nullptr;
    }
};

TEST_P(FnCorrectnessTest, SimpleRecursiveTypeTest) {
    VariantT<Suc, Nil> n = {};
    n.tag = 1ULL;
    VariantT<Suc, Nil> *wrapped_n = new VariantT<Suc, Nil>{n};

    VariantT<Suc, Nil> inner = {};
    inner.tag = 0ULL;
    reinterpret_cast<Suc *>(&inner.value)->value = wrapped_n;
    Nat *wrapped_inner = new Nat{inner};

    VariantT<Suc, Nil> outer = {};

    outer.tag = 0ULL;
    reinterpret_cast<Suc *>(&outer.value)->value = wrapped_inner;

    SimpleRecursiveTypeExample *fn = new SimpleRecursiveTypeExample{outer};

    WorkManager::run(fn);
    ASSERT_EQ(fn->ret.tag, inner.tag);
    ASSERT_EQ(reinterpret_cast<Suc *>(&fn->ret.value)->value,
              reinterpret_cast<Suc *>(&inner.value)->value);
}

using F = TupleT<Lazy<FnT<Int, Int>> *>;
struct SelfRecursiveFn : Closure<SelfRecursiveFn, F, Int, Int> {
    using Closure<SelfRecursiveFn, F, Int, Int>::Closure;
    FnT<Int, Int> g = nullptr;
    Lazy<Int> *body(Lazy<Int> *&x) override {
        WorkManager::await(x);
        if (x->value() > 0) {
            auto lz = std::get<0>(this->env);
            auto f = lz->value();
            if (g == nullptr) {
                g = f->clone();
                auto y = new LazyConstant<Int>{x->value() - 1};
                g->args = std::make_tuple(y);
                g->call();
            }
            return g;
        } else {
            return x;
        }
    }
};
TEST_P(FnCorrectnessTest, SelfRecursiveFnTest) {
    FnT<Int, Int> f = new SelfRecursiveFn{};
    dynamic_cast<SelfRecursiveFn *>(f)->env =
        std::make_tuple(new LazyConstant<FnT<Int, Int>>{f});
    Lazy<Int> *x = new LazyConstant<Int>{5};
    f->args = std::make_tuple(x);

    WorkManager::run(f);
    ASSERT_EQ(f->ret, 0);
}

const std::vector<unsigned> cpu_counts = {1, 2, 3, 4};
INSTANTIATE_TEST_SUITE_P(FnCorrectnessTests, FnCorrectnessTest,
                         ::testing::ValuesIn(cpu_counts));
