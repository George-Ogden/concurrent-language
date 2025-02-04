#pragma once

#include "data_structures/lazy.hpp"
#include "fn/fn.hpp"
#include "fn/operators.hpp"
#include "system/work_manager.hpp"
#include "types/builtin.hpp"
#include "types/compound.hpp"
#include "types/utils.hpp"

#include <gtest/gtest.h>

#include <memory>
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
    LazyT<Int> body(LazyT<Int> &x) override { return x; }
};

TEST_P(FnCorrectnessTest, IdentityTest) {
    Int x = 5;
    std::shared_ptr<IdentityInt> id = std::make_shared<IdentityInt>();
    id->args = std::make_tuple(std::make_shared<LazyConstant<Int>>(x));

    WorkManager::run(id);
    ASSERT_EQ(id->value(), 5);
}

struct FourWayPlusV1 : Closure<FourWayPlusV1, Empty, Int, Int, Int, Int, Int> {
    using Closure<FourWayPlusV1, Empty, Int, Int, Int, Int, Int>::Closure;
    FnT<Int, Int, Int> call1 = nullptr, call2 = nullptr, call3 = nullptr;
    LazyT<Int> body(LazyT<Int> &a, LazyT<Int> &b, LazyT<Int> &c,
                    LazyT<Int> &d) override {
        if (call1 == decltype(call1){}) {
            call1 = std::make_shared<Plus__BuiltIn>(a, b);
            WorkManager::call(call1);
        }
        if (call2 == decltype(call2){}) {
            call2 = std::make_shared<Plus__BuiltIn>(c, d);
            WorkManager::call(call2);
        }
        if (call3 == decltype(call3){}) {
            call3 = std::make_shared<Plus__BuiltIn>(call1, call2);
            WorkManager::call(call3);
        }
        return call3;
    }
};

struct FourWayPlusV2 : Closure<FourWayPlusV2, Empty, Int, Int, Int, Int, Int> {
    using Closure<FourWayPlusV2, Empty, Int, Int, Int, Int, Int>::Closure;
    std::shared_ptr<Plus__BuiltIn> call1 = nullptr, call2 = nullptr,
                                   call3 = nullptr;
    LazyT<Int> body(LazyT<Int> &a, LazyT<Int> &b, LazyT<Int> &c,
                    LazyT<Int> &d) override {
        if (call1 == decltype(call1){}) {
            call1 = std::make_shared<Plus__BuiltIn>(a, b);
            WorkManager::call(call1);
        }
        if (call2 == decltype(call2){}) {
            call2 = std::make_shared<Plus__BuiltIn>(call1, c);
            WorkManager::call(call2);
        }
        if (call3 == decltype(call3){}) {
            call3 = std::make_shared<Plus__BuiltIn>(call2, d);
            WorkManager::call(call3);
        }
        return call3;
    }
};

TEST_P(FnCorrectnessTest, FourWayPlusV1Test) {
    Int w = 11, x = 5, y = 10, z = 22;
    std::shared_ptr<FourWayPlusV1> plus = std::make_shared<FourWayPlusV1>();
    plus->args = std::make_tuple(std::make_shared<LazyConstant<Int>>(w),
                                 std::make_shared<LazyConstant<Int>>(x),
                                 std::make_shared<LazyConstant<Int>>(y),
                                 std::make_shared<LazyConstant<Int>>(z));

    WorkManager::run(plus);
    ASSERT_EQ(plus->value(), 48);
}

TEST_P(FnCorrectnessTest, FourWayPlusV2Test) {
    Int w = 11, x = 5, y = 10, z = 22;
    std::shared_ptr<FourWayPlusV2> plus = std::make_shared<FourWayPlusV2>();
    plus->args = std::make_tuple(std::make_shared<LazyConstant<Int>>(w),
                                 std::make_shared<LazyConstant<Int>>(x),
                                 std::make_shared<LazyConstant<Int>>(y),
                                 std::make_shared<LazyConstant<Int>>(z));

    WorkManager::run(plus);
    ASSERT_EQ(plus->value(), 48);
}

struct BranchingExample : EasyCloneFn<BranchingExample, Int, Int, Int, Int> {
    using EasyCloneFn<BranchingExample, Int, Int, Int, Int>::EasyCloneFn;
    std::shared_ptr<Comparison_GE__BuiltIn> call1 = nullptr;
    std::shared_ptr<Plus__BuiltIn> call2 = nullptr;
    std::shared_ptr<Minus__BuiltIn> call3 = nullptr;
    LazyT<Int> body(LazyT<Int> &x, LazyT<Int> &y, LazyT<Int> &z) override {
        if (call1 == decltype(call1){}) {
            call1 = std::make_shared<Comparison_GE__BuiltIn>(
                x, std::make_shared<LazyConstant<Int>>(0));
            WorkManager::call(call1);
        }
        WorkManager::await(call1);
        if (call1->value()) {
            if (call2 == decltype(call2){}) {
                call2 = std::make_shared<Plus__BuiltIn>(
                    y, std::make_shared<LazyConstant<Int>>(1));
                WorkManager::call(call2);
            }
        } else {
            if (call2 == decltype(call2){}) {
                call2 = std::make_shared<Plus__BuiltIn>(
                    z, std::make_shared<LazyConstant<Int>>(1));
                WorkManager::call(call2);
            }
        }
        if (call3 == decltype(call3){}) {
            call3 = std::make_shared<Minus__BuiltIn>(
                call2, std::make_shared<LazyConstant<Int>>(2));
            WorkManager::call(call3);
        }
        return call3;
    }
};

TEST_P(FnCorrectnessTest, PositiveBranchingExampleTest) {
    Int x = 5, y = 10, z = 22;
    std::shared_ptr<BranchingExample> branching =
        std::make_shared<BranchingExample>(x, y, z);

    WorkManager::run(branching);
    ASSERT_EQ(branching->value(), 9);
}

TEST_P(FnCorrectnessTest, NegativeBranchingExampleTest) {
    Int x = -5, y = 10, z = 22;
    std::shared_ptr<BranchingExample> branching =
        std::make_shared<BranchingExample>(x, y, z);

    WorkManager::run(branching);
    ASSERT_EQ(branching->value(), 21);
}

struct FlatBlockExample : EasyCloneFn<FlatBlockExample, Int, Int> {
    using EasyCloneFn<FlatBlockExample, Int, Int>::EasyCloneFn;
    FnT<Int, Int> call1 = nullptr;
    FnT<Int> block1 = nullptr;
    LazyT<Int> body(LazyT<Int> &x) override {
        if (block1 == decltype(block1){}) {
            block1 = std::make_shared<BlockFn<Int>>([&]() {
                if (call1 == decltype(call1){}) {
                    call1 = std::make_shared<Increment__BuiltIn>();
                    call1->args = std::make_tuple(x);
                    WorkManager::call(call1);
                }
                return call1;
            });
            WorkManager::call(block1);
        }
        block1->args = std::make_tuple();
        return block1;
    }
};

TEST_P(FnCorrectnessTest, FlatBlockExampleTest) {
    Int x = 5;
    std::shared_ptr<FlatBlockExample> block =
        std::make_shared<FlatBlockExample>(x);

    WorkManager::run(block);
    ASSERT_EQ(block->value(), 6);
}

struct NestedBlockExample : EasyCloneFn<NestedBlockExample, Int, Int> {
    using EasyCloneFn<NestedBlockExample, Int, Int>::EasyCloneFn;
    std::shared_ptr<Increment__BuiltIn> call1 = nullptr, call2 = nullptr,
                                        call3 = nullptr;
    FnT<Int> block1 = nullptr, block2 = nullptr, block3 = nullptr;
    LazyT<Int> body(LazyT<Int> &x) override {
        if (block1 == decltype(block1){}) {
            block1 = std::make_shared<BlockFn<Int>>([&]() {
                if (call1 == decltype(call1){}) {
                    call1 = std::make_shared<Increment__BuiltIn>(x);
                    WorkManager::call(call1);
                }
                if (block2 == decltype(block2){}) {
                    block2 = std::make_shared<BlockFn<Int>>([&]() {
                        if (call2 == decltype(call2){}) {
                            call2 = std::make_shared<Increment__BuiltIn>(call1);
                            WorkManager::call(call2);
                        }
                        if (block3 == decltype(block3){}) {
                            block3 = std::make_shared<BlockFn<Int>>([&] {
                                if (call3 == decltype(call3){}) {
                                    call3 =
                                        std::make_shared<Increment__BuiltIn>(
                                            call2);
                                    WorkManager::call(call3);
                                }
                                return call3;
                            });
                            WorkManager::call(block3);
                        }
                        return block3;
                    });
                    WorkManager::call(block2);
                }
                return block2;
            });
            WorkManager::call(block1);
        }
        return block1;
    }
};

TEST_P(FnCorrectnessTest, NestedBlockExampleTest) {
    Int x = 5;
    std::shared_ptr<NestedBlockExample> block =
        std::make_shared<NestedBlockExample>(x);

    WorkManager::run(block);
    ASSERT_EQ(block->value(), 8);
}

struct Adder : Closure<Adder, LazyT<Int>, Int, Int> {
    using Closure<Adder, LazyT<Int>, Int, Int>::Closure;
    FnT<Int, Int, Int> inner_res = nullptr;
    LazyT<Int> body(LazyT<Int> &x) override {
        if (inner_res == decltype(inner_res){}) {
            inner_res = std::make_shared<Plus__BuiltIn>(x, env);
            WorkManager::call(inner_res);
        }
        return inner_res;
    }
};

struct NestedFnExample : EasyCloneFn<NestedFnExample, Int, Int> {
    using EasyCloneFn<NestedFnExample, Int, Int>::EasyCloneFn;
    LazyT<FnT<Int, Int>> closure = nullptr;
    LazyT<Int> res = nullptr;
    LazyT<Int> body(LazyT<Int> &x) override {
        if (closure == decltype(closure){}) {
            closure = std::make_shared<
                LazyConstant<remove_lazy_t<decltype(closure)>>>(
                std::make_shared<Adder>());
            std::dynamic_pointer_cast<Adder>(closure->value())->env = x;
        }
        if (res == decltype(res){}) {
            WorkManager::await(closure);
            FnT<Int, Int> fn;
            std::tie(fn, res) = closure->value()->clone_with_args(x);
            WorkManager::call(fn);
        }
        return res;
    }
};

TEST_P(FnCorrectnessTest, NestedFnExampleTest) {
    Int x = 5;
    std::shared_ptr<NestedFnExample> nested =
        std::make_shared<NestedFnExample>(x);

    WorkManager::run(nested);
    ASSERT_EQ(nested->value(), 10);
}

struct IfStatementExample
    : EasyCloneFn<IfStatementExample, Int, Int, Int, Int> {
    using EasyCloneFn<IfStatementExample, Int, Int, Int, Int>::EasyCloneFn;
    std::shared_ptr<Comparison_GE__BuiltIn> call1 = nullptr;
    FnT<Int> branch1 = nullptr, branch2 = nullptr, branch = nullptr;
    std::shared_ptr<Plus__BuiltIn> call2_1 = nullptr, call2_2 = nullptr;
    std::shared_ptr<Minus__BuiltIn> call3 = nullptr;
    LazyT<Int> body(LazyT<Int> &x, LazyT<Int> &y, LazyT<Int> &z) override {
        if (branch1 == decltype(branch1){}) {
            branch1 = std::make_shared<BlockFn<Int>>([&]() {
                if (call2_1 == decltype(call2_1){}) {
                    call2_1 = std::make_shared<Plus__BuiltIn>(
                        y, std::make_shared<LazyConstant<Int>>(1));
                    WorkManager::call(call2_1);
                }
                return call2_1;
            });
        }
        if (branch2 == decltype(branch2){}) {
            branch2 = std::make_shared<BlockFn<Int>>([&]() {
                if (call2_2 == decltype(call2_2){}) {
                    call2_2 = std::make_shared<Plus__BuiltIn>(
                        z, std::make_shared<LazyConstant<Int>>(1));
                    WorkManager::call(call2_2);
                }
                return call2_2;
            });
        }

        if (call1 == decltype(call1){}) {
            call1 = std::make_shared<Comparison_GE__BuiltIn>(
                x, std::make_shared<LazyConstant<Int>>(0));
            WorkManager::call(call1);
        }
        WorkManager::await(call1);
        if (call1->value()) {
            if (branch == decltype(branch){}) {
                branch = branch1;
                WorkManager::call(branch);
            }
        } else {
            if (branch == decltype(branch){}) {
                branch = branch2;
                WorkManager::call(branch);
            };
        }
        if (call3 == decltype(call3){}) {
            call3 = std::make_shared<Minus__BuiltIn>(
                branch, std::make_shared<LazyConstant<Int>>(2));
            WorkManager::call(call3);
        }
        return call3;
    }
};

TEST_P(FnCorrectnessTest, IfStatementExampleTest) {
    Int x = 5, y = 10, z = 22;
    std::shared_ptr<IfStatementExample> branching =
        std::make_shared<IfStatementExample>(x, y, z);

    WorkManager::run(branching);
    ASSERT_EQ(branching->value(), 9);
}

struct SharedRegisterExample : EasyCloneFn<SharedRegisterExample, Int, Bool> {
    using EasyCloneFn<SharedRegisterExample, Int, Bool>::EasyCloneFn;
    LazyT<Int> body(LazyT<Bool> &b) override {
        WorkManager::await(b);
        LazyT<Int> m1;
        if (b->value()) {
            m1 = std::make_shared<LazyConstant<Int>>(1);
        } else {
            m1 = std::make_shared<LazyConstant<Int>>(0);
        }
        return m1;
    }
};

TEST_P(FnCorrectnessTest, SharedRegisterExampleTest) {
    {
        Bool b = true;
        std::shared_ptr<SharedRegisterExample> example =
            std::make_shared<SharedRegisterExample>(b);

        WorkManager::run(example);
        ASSERT_EQ(example->value(), 1);
    }
    {
        Bool b = false;
        std::shared_ptr<SharedRegisterExample> example =
            std::make_shared<SharedRegisterExample>(b);

        WorkManager::run(example);
        ASSERT_EQ(example->value(), 0);
    }
}

struct RecursiveDouble : EasyCloneFn<RecursiveDouble, Int, Int> {
    using EasyCloneFn<RecursiveDouble, Int, Int>::EasyCloneFn;
    std::shared_ptr<RecursiveDouble> call1 = nullptr, call3 = nullptr;
    std::shared_ptr<Plus__BuiltIn> call2 = nullptr;
    LazyT<Int> body(LazyT<Int> &x) override {
        WorkManager::await(x);
        if (x->value() > 0) {
            if (call1 == decltype(call1){}) {
                auto arg = std::make_shared<Decrement__BuiltIn>(x);
                WorkManager::call(arg);
                call1 = std::make_shared<RecursiveDouble>(arg);
                WorkManager::call(call1);
            }

            if (call3 == decltype(call3){}) {
                auto arg = std::make_shared<Decrement__BuiltIn>(x);
                WorkManager::call(arg);
                call3 = std::make_shared<RecursiveDouble>(arg);
                call3->run();
            }

            if (call2 == decltype(call2){}) {
                call2 = std::make_shared<Plus__BuiltIn>(
                    call1, std::make_shared<LazyConstant<Int>>(2));
                WorkManager::call(call2);
            }
            return call2;
        } else {
            return std::make_shared<LazyConstant<Int>>(0);
        }
    }
};

TEST_P(FnCorrectnessTest, RecursiveDoubleTest1) {
    Int x = 2;
    std::shared_ptr<RecursiveDouble> double_ =
        std::make_shared<RecursiveDouble>(x);

    WorkManager::run(double_);
    ASSERT_EQ(double_->value(), 4);
}

TEST_P(FnCorrectnessTest, RecursiveDoubleTest2) {
    Int x = -5;
    std::shared_ptr<RecursiveDouble> double_ =
        std::make_shared<RecursiveDouble>(x);

    WorkManager::run(double_);
    ASSERT_EQ(double_->value(), 0);
}

struct EvenOrOdd : EasyCloneFn<EvenOrOdd, Bool, Int> {
    using EasyCloneFn<EvenOrOdd, Bool, Int>::EasyCloneFn;
    LazyT<Bool> body(LazyT<Int> &x) override {
        WorkManager::await(x);
        return std::make_shared<LazyConstant<Bool>>(x->value() & 1);
    }
};

struct ApplyIntBool : EasyCloneFn<ApplyIntBool, Bool, FnT<Bool, Int>, Int> {
    using EasyCloneFn<ApplyIntBool, Bool, FnT<Bool, Int>, Int>::EasyCloneFn;
    LazyT<Bool> body(LazyT<FnT<Bool, Int>> &f, LazyT<Int> &x) override {
        WorkManager::await(f);
        auto g = f->value()->clone();
        g->args = std::make_tuple(x);
        WorkManager::call(g);
        return g;
    }
};

TEST_P(FnCorrectnessTest, HigherOrderFunctionTest) {
    FnT<Bool, Int> f = std::make_shared<EvenOrOdd>();
    Int x = 5;
    std::shared_ptr<ApplyIntBool> apply = std::make_shared<ApplyIntBool>(f, x);

    WorkManager::run(apply);
    ASSERT_TRUE(apply->value());
}

struct PairIntBool
    : EasyCloneFn<PairIntBool, TupleT<Int, TupleT<Bool>>, Int, Bool> {
    using EasyCloneFn<PairIntBool, TupleT<Int, TupleT<Bool>>, Int,
                      Bool>::EasyCloneFn;
    LazyT<TupleT<Int, TupleT<Bool>>> body(LazyT<Int> &x,
                                          LazyT<Bool> &y) override {
        return std::make_tuple(x, std::make_tuple(y));
    }
};

TEST_P(FnCorrectnessTest, TupleTest) {
    Int x = 5;
    Bool y = true;
    std::shared_ptr<PairIntBool> pair = std::make_shared<PairIntBool>(x, y);

    WorkManager::run(pair);
    ASSERT_EQ(std::get<0>(pair->value()), 5);
    ASSERT_EQ(std::get<0>(std::get<1>(pair->value())), true);
}

struct HigherOrderReuse
    : EasyCloneFn<HigherOrderReuse, Int, FnT<Int, Int>, Int, Int> {
    using EasyCloneFn<HigherOrderReuse, Int, FnT<Int, Int>, Int,
                      Int>::EasyCloneFn;
    FnT<Int, Int> call1 = nullptr, call2 = nullptr;
    std::shared_ptr<Plus__BuiltIn> call3 = nullptr;
    LazyT<Int> body(LazyT<FnT<Int, Int>> &f, LazyT<Int> &x,
                    LazyT<Int> &y) override {
        WorkManager::await(f);
        if (call1 == decltype(call1){}) {
            call1 = f->value()->clone();
            call1->args = std::make_tuple(x);
            WorkManager::call(call1);
        }
        if (call2 == decltype(call2){}) {
            call2 = f->value()->clone();
            call2->args = std::make_tuple(y);
            WorkManager::call(call2);
        }
        if (call3 == decltype(call3){}) {
            call3 = std::make_shared<Plus__BuiltIn>(call1, call2);
            WorkManager::call(call3);
        }
        return call3;
    }
};

TEST_P(FnCorrectnessTest, ReusedHigherOrderFunctionTest) {
    FnT<Int, Int> f = std::make_shared<Increment__BuiltIn>();
    Int x = 5, y = 4;
    std::shared_ptr<HigherOrderReuse> F =
        std::make_shared<HigherOrderReuse>(f, x, y);

    WorkManager::run(F);
    ASSERT_EQ(F->value(), 11);
}

struct Twoo;
struct Faws;
typedef VariantT<Twoo, Faws> Bull;
struct Twoo {};
struct Faws {};

struct BoolUnion : EasyCloneFn<BoolUnion, Bool, Bull> {
    using EasyCloneFn<BoolUnion, Bool, Bull>::EasyCloneFn;
    LazyT<Bool> body(LazyT<Bull> &x) override {
        WorkManager::await(x);
        return std::make_shared<LazyConstant<Bool>>(x->value().tag == 0);
    }
};

TEST_P(FnCorrectnessTest, ValueFreeUnionTest) {
    {
        Bull bull{};
        bull.tag = 0;
        std::shared_ptr<BoolUnion> fn = std::make_shared<BoolUnion>(bull);

        WorkManager::run(fn);
        ASSERT_TRUE(fn->value());
    }

    {
        Bull bull{};
        bull.tag = 1ULL;
        std::shared_ptr<BoolUnion> fn = std::make_shared<BoolUnion>(bull);

        WorkManager::run(fn);
        ASSERT_FALSE(fn->value());
    }
}

struct Left;
struct Right;
typedef VariantT<Left, Right> EitherIntBool;
struct Left {
    using type = LazyT<Int>;
    type value;
};
struct Right {
    using type = LazyT<Bool>;
    type value;
};

struct EitherIntBoolExtractor
    : EasyCloneFn<EitherIntBoolExtractor, Bool, EitherIntBool> {
    using EasyCloneFn<EitherIntBoolExtractor, Bool, EitherIntBool>::EasyCloneFn;
    LazyT<Bool> body(LazyT<EitherIntBool> &either) override {
        WorkManager::await(either);
        EitherIntBool x = either->value();
        switch (x.tag) {
        case 0ULL: {
            auto left = reinterpret_cast<Left *>(&x.value)->value;
            WorkManager::await(left);
            return std::make_shared<LazyConstant<Bool>>(left->value() > 10);
        }
        case 1ULL: {
            auto right = reinterpret_cast<Right *>(&x.value)->value;
            WorkManager::await(right);
            return std::make_shared<LazyConstant<Bool>>(right->value());
        }
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
            new (&either.value)
                Left{std::make_shared<LazyConstant<Int>>(value)};
        } else {
            new (&either.value)
                Right{std::make_shared<LazyConstant<Bool>>(value)};
        }

        std::shared_ptr<EitherIntBoolExtractor> fn =
            std::make_shared<EitherIntBoolExtractor>(either);

        WorkManager::run(fn);
        ASSERT_EQ(fn->value(), result);
    }
}

struct EitherIntBoolEdgeCase
    : EasyCloneFn<EitherIntBoolEdgeCase, Bool, EitherIntBool> {
    using EasyCloneFn<EitherIntBoolEdgeCase, Bool, EitherIntBool>::EasyCloneFn;
    LazyT<Bool> y = nullptr;
    LazyT<Bool> body(LazyT<EitherIntBool> &either) override {
        WorkManager::await(either);
        EitherIntBool x = either->value();
        switch (x.tag) {
        case 0ULL: {
            LazyT<Left::type> i = reinterpret_cast<Left *>(&x.value)->value;
            LazyT<Int> z = std::make_shared<LazyConstant<Int>>(0);
            if (y == decltype(y){}) {
                y = std::make_shared<Comparison_GT__BuiltIn>();
                dynamic_fn_cast<FnT<Bool, Int, Int>>(y)->args =
                    std::make_tuple(i, z);
                WorkManager::call(dynamic_fn_cast(y));
            }
            break;
        }
        case 1ULL: {
            LazyT<Right::type> b = reinterpret_cast<Right *>(&x.value)->value;
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

            new (&either.value)
                Left{std::make_shared<LazyConstant<Int>>(value)};
        } else {
            new (&either.value)
                Right{std::make_shared<LazyConstant<Bool>>(value)};
        }

        std::shared_ptr<EitherIntBoolEdgeCase> fn =
            std::make_shared<EitherIntBoolEdgeCase>(either);

        WorkManager::run(fn);
        ASSERT_EQ(fn->value(), result);
    }
}

struct Cons;
struct Nil;
typedef VariantT<Cons, Nil> ListInt;
struct Cons {
    using type = TupleT<Int, ListInt>;
    LazyT<type> value;
};
struct Nil {
    Empty value;
};

struct ListIntSum : EasyCloneFn<ListIntSum, Int, ListInt> {
    using EasyCloneFn<ListIntSum, Int, ListInt>::EasyCloneFn;
    std::shared_ptr<ListIntSum> call1 = nullptr;
    std::shared_ptr<Plus__BuiltIn> call2 = nullptr;
    LazyT<Int> body(LazyT<ListInt> &lazy_list) override {
        WorkManager::await(lazy_list);
        ListInt list = lazy_list->value();
        switch (list.tag) {
        case 0: {
            LazyT<Cons::type> cons =
                reinterpret_cast<Cons *>(&list.value)->value;
            WorkManager::await(cons);
            LazyT<Int> head = std::get<0ULL>(cons);
            LazyT<ListInt> tail = std::get<1ULL>(cons);

            if (call1 == decltype(call1){}) {
                call1 = std::make_shared<ListIntSum>();
                call1->args = std::make_tuple(tail);
                WorkManager::call(call1);
            }

            if (call2 == decltype(call2){}) {
                call2 = std::make_shared<Plus__BuiltIn>();
                call2->args = std::make_tuple(call1, head);
                WorkManager::call(call2);
            }
            return call2;
        }
        case 1:
            return std::make_shared<LazyConstant<Int>>(0);
        }
        return nullptr;
    }
};

TEST_P(FnCorrectnessTest, RecursiveTypeTest) {
    LazyT<ListInt> tail;
    tail = std::make_shared<LazyConstant<remove_lazy_t<decltype(tail)>>>(
        std::integral_constant<std::size_t, 1>(), Nil{});
    LazyT<ListInt> third;
    third = std::make_shared<LazyConstant<remove_lazy_t<decltype(third)>>>(
        std::integral_constant<std::size_t, 0>(),
        Cons{std::make_tuple(std::make_shared<LazyConstant<Int>>(8), tail)});
    LazyT<ListInt> second;
    second = std::make_shared<LazyConstant<remove_lazy_t<decltype(second)>>>(
        std::integral_constant<std::size_t, 0>(),
        Cons{std::make_tuple(std::make_shared<LazyConstant<Int>>(4), third)});
    LazyT<ListInt> first;
    first = std::make_shared<LazyConstant<remove_lazy_t<decltype(first)>>>(
        std::integral_constant<std::size_t, 0>(),
        Cons{std::make_tuple(std::make_shared<LazyConstant<Int>>(-9), second)});

    std::shared_ptr<ListIntSum> summer = std::make_shared<ListIntSum>(first);
    WorkManager::run(summer);
    ASSERT_EQ(summer->value(), 3);
}

struct Suc;
typedef VariantT<Suc, Nil> Nat;
struct Suc {
    using type = Nat;
    LazyT<type> value;
};

struct SimpleRecursiveTypeExample
    : EasyCloneFn<SimpleRecursiveTypeExample, VariantT<Suc, Nil>,
                  VariantT<Suc, Nil>> {
    using EasyCloneFn<SimpleRecursiveTypeExample, VariantT<Suc, Nil>,
                      VariantT<Suc, Nil>>::EasyCloneFn;
    LazyT<VariantT<Suc, Nil>> body(LazyT<VariantT<Suc, Nil>> &nat) override {
        WorkManager::await(nat);
        VariantT<Suc, Nil> nat_ = nat->value();
        switch (nat_.tag) {
        case 0: {
            LazyT<Suc::type> s = reinterpret_cast<Suc *>(&nat_.value)->value;
            return s;
        }
        case 1: {
            VariantT<Suc, Nil> n = {};
            n.tag = 1ULL;
            return std::make_shared<LazyConstant<VariantT<Suc, Nil>>>(n);
        }
        }
        return nullptr;
    }
};

TEST_P(FnCorrectnessTest, SimpleRecursiveTypeTest) {
    LazyT<VariantT<Suc, Nil>> n =
        std::make_shared<LazyConstant<VariantT<Suc, Nil>>>(
            std::integral_constant<std::size_t, 1>());
    LazyT<VariantT<Suc, Nil>> inner =
        std::make_shared<LazyConstant<VariantT<Suc, Nil>>>(
            std::integral_constant<std::size_t, 0>(), Suc{n});
    LazyT<VariantT<Suc, Nil>> outer =
        std::make_shared<LazyConstant<VariantT<Suc, Nil>>>(
            std::integral_constant<std::size_t, 0>(), Suc{inner});

    std::shared_ptr<SimpleRecursiveTypeExample> fn =
        std::make_shared<SimpleRecursiveTypeExample>(outer);

    WorkManager::run(fn);

    auto res = fn->value();
    ASSERT_EQ(res.tag, inner->value().tag);
    auto tmp = inner->value().value;
    ASSERT_EQ(reinterpret_cast<Suc *>(&res.value)->value,
              reinterpret_cast<Suc *>(&tmp)->value);
}

using F = LazyT<TupleT<FnT<Int, Int>>>;
struct SelfRecursiveFn : Closure<SelfRecursiveFn, F, Int, Int> {
    using Closure<SelfRecursiveFn, F, Int, Int>::Closure;
    FnT<Int, Int> g = nullptr;
    LazyT<Int> body(LazyT<Int> &x) override {
        WorkManager::await(x);
        if (x->value() > 0) {
            auto lz = std::get<0>(this->env);
            auto f = lz->value();
            if (g == decltype(g){}) {
                g = f->clone();
                auto y = std::make_shared<LazyConstant<Int>>(x->value() - 1);
                g->args = std::make_tuple(y);
                WorkManager::call(g);
            }
            return g;
        } else {
            return x;
        }
    }
};

TEST_P(FnCorrectnessTest, SelfRecursiveFnTest) {
    FnT<Int, Int> f = std::make_shared<SelfRecursiveFn>();
    std::dynamic_pointer_cast<SelfRecursiveFn>(f)->env =
        std::make_tuple(std::make_shared<LazyConstant<FnT<Int, Int>>>(f));
    LazyT<Int> x = std::make_shared<LazyConstant<Int>>(5);
    f->args = std::make_tuple(x);

    WorkManager::run(f);
    ASSERT_EQ(f->value(), 0);
}

template <typename T, typename U>
struct MakePairFn : Closure<MakePairFn<T, U>, Empty, TupleT<T, U>, T, U> {
    using Closure<MakePairFn<T, U>, Empty, TupleT<T, U>, T, U>::Closure;
    LazyT<TupleT<T, U>> body(LazyT<T> &x, LazyT<U> &y) override {
        return std::make_tuple(x, y);
    }
};

TEST_P(FnCorrectnessTest, MakePairFnTest) {
    FnT<TupleT<Int, Int>, Int, Int> pair_maker =
        std::make_shared<MakePairFn<Int, Int>>();
    LazyT<Int> x = std::make_shared<LazyConstant<Int>>(0),
               y = std::make_shared<LazyConstant<Int>>(1);
    LazyT<TupleT<Int, Int>> result;
    std::shared_ptr<Fn> fn;
    std::tie(fn, result) = pair_maker->clone_with_args(x, y);

    WorkManager::run(fn);
    ASSERT_EQ(std::get<0>(result)->value(), 0);
    ASSERT_EQ(std::get<1>(result)->value(), 1);
}

struct PairSumFn : Closure<PairSumFn, Empty, Int, Int, Int> {
    using Closure<PairSumFn, Empty, Int, Int, Int>::Closure;
    LazyT<TupleT<Int, Int>> pair;
    LazyT<Int> res;
    LazyT<Int> body(LazyT<Int> &x, LazyT<Int> &y) override {
        LazyT<FnT<TupleT<Int, Int>, Int, Int>> make_pair =
            std::make_shared<LazyConstant<remove_lazy_t<decltype(make_pair)>>>(
                std::make_shared<MakePairFn<Int, Int>>());
        if (pair == decltype(pair){}) {
            std::shared_ptr<Fn> fn;
            std::tie(fn, pair) = make_pair->value()->clone_with_args(x, y);
            WorkManager::call(fn);
        }
        LazyT<Int> a = std::get<0>(pair);
        LazyT<Int> b = std::get<1>(pair);
        if (res == decltype(res){}) {
            res = std::make_shared<Plus__BuiltIn>(a, b);
            WorkManager::call(dynamic_fn_cast(res));
        }
        return res;
    }
};

TEST_P(FnCorrectnessTest, PairSumTest) {
    FnT<TupleT<Int, Int>, Int, Int> pair_maker =
        std::make_shared<MakePairFn<Int, Int>>();
    LazyT<Int> x = std::make_shared<LazyConstant<Int>>(0),
               y = std::make_shared<LazyConstant<Int>>(1);
    LazyT<TupleT<Int, Int>> result;
    std::shared_ptr<Fn> fn;
    std::tie(fn, result) = pair_maker->clone_with_args(x, y);

    WorkManager::run(fn);
    ASSERT_EQ(std::get<0>(result)->value(), 0);
    ASSERT_EQ(std::get<1>(result)->value(), 1);
}

struct TupleAddFn : Closure<TupleAddFn, Empty, TupleT<Int, Int>, Int, Int> {
    using Closure<TupleAddFn, Empty, TupleT<Int, Int>, Int, Int>::Closure;
    std::shared_ptr<Plus__BuiltIn> plus = nullptr;
    std::shared_ptr<Minus__BuiltIn> minus = nullptr;
    LazyT<TupleT<Int, Int>> body(LazyT<Int> &x, LazyT<Int> &y) override {
        if (plus == decltype(plus){}) {
            plus = std::make_shared<Plus__BuiltIn>();
            plus->args = std::make_tuple(x, y);
            WorkManager::call(plus);
        }
        if (minus == decltype(minus){}) {
            minus = std::make_shared<Minus__BuiltIn>();
            minus->args = std::make_tuple(x, y);
            WorkManager::call(minus);
        }
        return std::make_tuple(plus, minus);
    }
};

TEST_P(FnCorrectnessTest, TupleAddFnTest) {
    FnT<TupleT<Int, Int>, Int, Int> tuple_fn = std::make_shared<TupleAddFn>();
    LazyT<Int> x = std::make_shared<LazyConstant<Int>>(2),
               y = std::make_shared<LazyConstant<Int>>(9);
    LazyT<TupleT<Int, Int>> result;
    std::shared_ptr<Fn> fn;
    std::tie(fn, result) = tuple_fn->clone_with_args(x, y);

    WorkManager::run(fn);
    ASSERT_EQ(std::get<0>(result)->value(), 11);
    ASSERT_EQ(std::get<1>(result)->value(), -7);
}

const std::vector<unsigned> cpu_counts = {1, 2, 3, 4};
INSTANTIATE_TEST_SUITE_P(FnCorrectnessTests, FnCorrectnessTest,
                         ::testing::ValuesIn(cpu_counts));
