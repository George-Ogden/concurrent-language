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
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &x) override {
        return x;
    }
};

TEST_P(FnCorrectnessTest, IdentityTest) {
    Int x = 5;
    std::shared_ptr<IdentityInt> id = std::make_shared<IdentityInt>();
    id->args = std::make_tuple(std::make_shared<LazyConstant<Int>>(x));

    WorkManager::run(id);
    ASSERT_EQ(id->ret, 5);
}

struct FourWayPlusV1 : Closure<FourWayPlusV1, Empty, Int, Int, Int, Int, Int> {
    using Closure<FourWayPlusV1, Empty, Int, Int, Int, Int, Int>::Closure;
    FnT<Int, Int, Int> call1 = nullptr, call2 = nullptr, call3 = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &a,
                                    std::shared_ptr<Lazy<Int>> &b,
                                    std::shared_ptr<Lazy<Int>> &c,
                                    std::shared_ptr<Lazy<Int>> &d) override {
        if (call1 == nullptr) {
            call1 = std::make_shared<Plus__BuiltIn>();
            call1->args = std::make_tuple(a, b);
            WorkManager::call(call1);
        }
        if (call2 == nullptr) {
            call2 = std::make_shared<Plus__BuiltIn>();
            call2->args = std::make_tuple(c, d);
            WorkManager::call(call2);
        }
        if (call3 == nullptr) {
            call3 = std::make_shared<Plus__BuiltIn>();
            call3->args = std::make_tuple(call1, call2);
            WorkManager::call(call3);
        }
        return call3;
    }
};

struct FourWayPlusV2 : Closure<FourWayPlusV2, Empty, Int, Int, Int, Int, Int> {
    using Closure<FourWayPlusV2, Empty, Int, Int, Int, Int, Int>::Closure;
    std::shared_ptr<Plus__BuiltIn> call1 = nullptr, call2 = nullptr,
                                   call3 = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &a,
                                    std::shared_ptr<Lazy<Int>> &b,
                                    std::shared_ptr<Lazy<Int>> &c,
                                    std::shared_ptr<Lazy<Int>> &d) override {
        if (call1 == nullptr) {
            call1 = std::make_shared<Plus__BuiltIn>(a, b);
            WorkManager::call(call1);
        }
        if (call2 == nullptr) {
            call2 = std::make_shared<Plus__BuiltIn>(call1, c);
            WorkManager::call(call2);
        }
        if (call3 == nullptr) {
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
    ASSERT_EQ(plus->ret, 48);
}

TEST_P(FnCorrectnessTest, FourWayPlusV2Test) {
    Int w = 11, x = 5, y = 10, z = 22;
    std::shared_ptr<FourWayPlusV2> plus = std::make_shared<FourWayPlusV2>();
    plus->args = std::make_tuple(std::make_shared<LazyConstant<Int>>(w),
                                 std::make_shared<LazyConstant<Int>>(x),
                                 std::make_shared<LazyConstant<Int>>(y),
                                 std::make_shared<LazyConstant<Int>>(z));

    WorkManager::run(plus);
    ASSERT_EQ(plus->ret, 48);
}

struct BranchingExample : EasyCloneFn<BranchingExample, Int, Int, Int, Int> {
    using EasyCloneFn<BranchingExample, Int, Int, Int, Int>::EasyCloneFn;
    std::shared_ptr<Comparison_GE__BuiltIn> call1 = nullptr;
    std::shared_ptr<Plus__BuiltIn> call2 = nullptr;
    std::shared_ptr<Minus__BuiltIn> call3 = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &x,
                                    std::shared_ptr<Lazy<Int>> &y,
                                    std::shared_ptr<Lazy<Int>> &z) override {
        if (call1 == nullptr) {
            call1 = std::make_shared<Comparison_GE__BuiltIn>(
                x, std::make_shared<LazyConstant<Int>>(0));
            WorkManager::call(call1);
        }
        WorkManager::await(call1);
        call1->run();
        if (call1->value()) {
            if (call2 == nullptr) {
                call2 = std::make_shared<Plus__BuiltIn>(
                    y, std::make_shared<LazyConstant<Int>>(1));
                WorkManager::call(call2);
            }
        } else {
            if (call2 == nullptr) {
                call2 = std::make_shared<Plus__BuiltIn>(
                    z, std::make_shared<LazyConstant<Int>>(1));
                WorkManager::call(call2);
            }
        }
        if (call3 == nullptr) {
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
    ASSERT_EQ(branching->ret, 9);
}

TEST_P(FnCorrectnessTest, NegativeBranchingExampleTest) {
    Int x = -5, y = 10, z = 22;
    std::shared_ptr<BranchingExample> branching =
        std::make_shared<BranchingExample>(x, y, z);

    WorkManager::run(branching);
    ASSERT_EQ(branching->ret, 21);
}

struct FlatBlockExample : EasyCloneFn<FlatBlockExample, Int, Int> {
    using EasyCloneFn<FlatBlockExample, Int, Int>::EasyCloneFn;
    FnT<Int, Int> call1 = nullptr;
    FnT<Int> block1 = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &x) override {
        if (block1 == nullptr) {
            block1 = std::make_shared<BlockFn<Int>>([&]() {
                if (call1 == nullptr) {
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
    ASSERT_EQ(block->ret, 6);
}

struct NestedBlockExample : EasyCloneFn<NestedBlockExample, Int, Int> {
    using EasyCloneFn<NestedBlockExample, Int, Int>::EasyCloneFn;
    std::shared_ptr<Increment__BuiltIn> call1 = nullptr, call2 = nullptr,
                                        call3 = nullptr;
    FnT<Int> block1 = nullptr, block2 = nullptr, block3 = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &x) override {
        if (block1 == nullptr) {
            block1 = std::make_shared<BlockFn<Int>>([&]() {
                if (call1 == nullptr) {
                    call1 = std::make_shared<Increment__BuiltIn>(x);
                    WorkManager::call(call1);
                }
                if (block2 == nullptr) {
                    block2 = std::make_shared<BlockFn<Int>>([&]() {
                        if (call2 == nullptr) {
                            call2 = std::make_shared<Increment__BuiltIn>(call1);
                            WorkManager::call(call2);
                        }
                        if (block3 == nullptr) {
                            block3 = std::make_shared<BlockFn<Int>>([&] {
                                if (call3 == nullptr) {
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
    ASSERT_EQ(block->ret, 8);
}

struct Adder : Closure<Adder, std::shared_ptr<Lazy<Int>>, Int, Int> {
    using Closure<Adder, std::shared_ptr<Lazy<Int>>, Int, Int>::Closure;
    FnT<Int, Int, Int> inner_res = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &x) override {
        if (inner_res == nullptr) {
            inner_res = std::make_shared<Plus__BuiltIn>(x, env);
            WorkManager::call(inner_res);
        }
        return inner_res;
    }
};

struct NestedFnExample : EasyCloneFn<NestedFnExample, Int, Int> {
    using EasyCloneFn<NestedFnExample, Int, Int>::EasyCloneFn;
    FnT<Int, Int> closure = nullptr;
    FnT<Int, Int> res = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &x) override {
        if (closure == nullptr) {
            closure = std::make_shared<Adder>(x);
        }
        if (res == nullptr) {
            res = closure->clone();
            res->args = std::make_tuple(x);
            WorkManager::call(res);
        }
        return res;
    }
};

TEST_P(FnCorrectnessTest, NestedFnExampleTest) {
    Int x = 5;
    std::shared_ptr<NestedFnExample> nested =
        std::make_shared<NestedFnExample>(x);

    WorkManager::run(nested);
    ASSERT_EQ(nested->ret, 10);
}

struct IfStatementExample
    : EasyCloneFn<IfStatementExample, Int, Int, Int, Int> {
    using EasyCloneFn<IfStatementExample, Int, Int, Int, Int>::EasyCloneFn;
    std::shared_ptr<Comparison_GE__BuiltIn> call1 = nullptr;
    FnT<Int> branch1 = nullptr, branch2 = nullptr, branch = nullptr;
    std::shared_ptr<Plus__BuiltIn> call2_1 = nullptr, call2_2 = nullptr;
    std::shared_ptr<Minus__BuiltIn> call3 = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &x,
                                    std::shared_ptr<Lazy<Int>> &y,
                                    std::shared_ptr<Lazy<Int>> &z) override {
        if (branch1 == nullptr) {
            branch1 = std::make_shared<BlockFn<Int>>([&]() {
                if (call2_1 == nullptr) {
                    call2_1 = std::make_shared<Plus__BuiltIn>(
                        y, std::make_shared<LazyConstant<Int>>(1));
                    WorkManager::call(call2_1);
                }
                return call2_1;
            });
        }
        if (branch2 == nullptr) {
            branch2 = std::make_shared<BlockFn<Int>>([&]() {
                if (call2_2 == nullptr) {
                    call2_2 = std::make_shared<Plus__BuiltIn>(
                        z, std::make_shared<LazyConstant<Int>>(1));
                    WorkManager::call(call2_2);
                }
                return call2_2;
            });
        }

        if (call1 == nullptr) {
            call1 = std::make_shared<Comparison_GE__BuiltIn>(
                x, std::make_shared<LazyConstant<Int>>(0));
            WorkManager::call(call1);
        }
        WorkManager::await(call1);
        if (call1->value()) {
            if (branch == nullptr) {
                branch = branch1;
                WorkManager::call(branch);
            }
        } else {
            if (branch == nullptr) {
                branch = branch2;
                WorkManager::call(branch);
            };
        }
        if (call3 == nullptr) {
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
    ASSERT_EQ(branching->ret, 9);
}

struct SharedRegisterExample : EasyCloneFn<SharedRegisterExample, Int, Bool> {
    using EasyCloneFn<SharedRegisterExample, Int, Bool>::EasyCloneFn;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Bool>> &b) override {
        WorkManager::await(b);
        Bool m0;
        Int m1;
        m0 = b->value();
        if (m0) {
            m1 = 1;
        } else {
            m1 = 0;
        }
        std::shared_ptr<Lazy<Int>> m2 = std::make_shared<LazyConstant<Int>>(m1);
        return m2;
    }
};

TEST_P(FnCorrectnessTest, SharedRegisterExampleTest) {
    Bool b = true;
    std::shared_ptr<SharedRegisterExample> example =
        std::make_shared<SharedRegisterExample>(b);

    WorkManager::run(example);
    ASSERT_EQ(example->ret, 1);
}

struct RecursiveDouble : EasyCloneFn<RecursiveDouble, Int, Int> {
    using EasyCloneFn<RecursiveDouble, Int, Int>::EasyCloneFn;
    std::shared_ptr<RecursiveDouble> call1 = nullptr, call3 = nullptr;
    std::shared_ptr<Plus__BuiltIn> call2 = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &x) override {
        WorkManager::await(x);
        if (x->value() > 0) {
            if (call1 == nullptr) {
                auto arg = std::make_shared<Decrement__BuiltIn>(x);
                WorkManager::call(arg);
                call1 = std::make_shared<RecursiveDouble>(arg);
                WorkManager::call(call1);
            }

            if (call3 == nullptr) {
                auto arg = std::make_shared<Decrement__BuiltIn>(x);
                WorkManager::call(arg);
                call3 = std::make_shared<RecursiveDouble>(arg);
                call3->run();
            }

            if (call2 == nullptr) {
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
    ASSERT_EQ(double_->ret, 4);
}

TEST_P(FnCorrectnessTest, RecursiveDoubleTest2) {
    Int x = -5;
    std::shared_ptr<RecursiveDouble> double_ =
        std::make_shared<RecursiveDouble>(x);

    WorkManager::run(double_);
    ASSERT_EQ(double_->ret, 0);
}

struct EvenOrOdd : EasyCloneFn<EvenOrOdd, Bool, Int> {
    using EasyCloneFn<EvenOrOdd, Bool, Int>::EasyCloneFn;
    std::shared_ptr<Lazy<Bool>> body(std::shared_ptr<Lazy<Int>> &x) override {
        WorkManager::await(x);
        return std::make_shared<LazyConstant<Bool>>(x->value() & 1);
    }
};

struct ApplyIntBool : EasyCloneFn<ApplyIntBool, Bool, FnT<Bool, Int>, Int> {
    using EasyCloneFn<ApplyIntBool, Bool, FnT<Bool, Int>, Int>::EasyCloneFn;
    std::shared_ptr<Lazy<Bool>> body(std::shared_ptr<Lazy<FnT<Bool, Int>>> &f,
                                     std::shared_ptr<Lazy<Int>> &x) override {
        WorkManager::await(f);
        auto g = f->value();
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
    ASSERT_TRUE(apply->ret);
}

struct PairIntBool : EasyCloneFn<PairIntBool, TupleT<Int, Bool>, Int, Bool> {
    using EasyCloneFn<PairIntBool, TupleT<Int, Bool>, Int, Bool>::EasyCloneFn;
    std::shared_ptr<Lazy<TupleT<Int, Bool>>>
    body(std::shared_ptr<Lazy<Int>> &x,
         std::shared_ptr<Lazy<Bool>> &y) override {
        WorkManager::await(x, y);
        return std::make_shared<LazyConstant<TupleT<Int, Bool>>>(
            std::make_tuple(x->value(), y->value()));
    }
};

struct HigherOrderReuse
    : EasyCloneFn<HigherOrderReuse, Int, FnT<Int, Int>, Int, Int> {
    using EasyCloneFn<HigherOrderReuse, Int, FnT<Int, Int>, Int,
                      Int>::EasyCloneFn;
    FnT<Int, Int> call1 = nullptr, call2 = nullptr;
    std::shared_ptr<Plus__BuiltIn> call3 = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<FnT<Int, Int>>> &f,
                                    std::shared_ptr<Lazy<Int>> &x,
                                    std::shared_ptr<Lazy<Int>> &y) override {
        WorkManager::await(f);
        if (call1 == nullptr) {
            call1 = f->value();
            call1->args = std::make_tuple(x);
            WorkManager::call(call1);
        }
        if (call2 == nullptr) {
            call2 = call1->clone();
            call2->args = std::make_tuple(y);
            WorkManager::call(call2);
        }
        if (call3 == nullptr) {
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
    ASSERT_EQ(F->ret, 11);
}

TEST_P(FnCorrectnessTest, TupleTest) {
    Int x = 5;
    Bool y = true;
    std::shared_ptr<PairIntBool> pair = std::make_shared<PairIntBool>(x, y);

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
    std::shared_ptr<Lazy<Bool>> body(std::shared_ptr<Lazy<Bull>> &x) override {
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
        ASSERT_TRUE(fn->ret);
    }

    {
        Bull bull{};
        bull.tag = 1ULL;
        std::shared_ptr<BoolUnion> fn = std::make_shared<BoolUnion>(bull);

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
    std::shared_ptr<Lazy<Bool>>
    body(std::shared_ptr<Lazy<EitherIntBool>> &either) override {
        WorkManager::await(either);
        EitherIntBool x = either->value();
        switch (x.tag) {
        case 0ULL:
            return std::make_shared<LazyConstant<Bool>>(
                reinterpret_cast<Left *>(&x.value)->value > 10);
        case 1ULL:
            return std::make_shared<LazyConstant<Bool>>(
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

        std::shared_ptr<EitherIntBoolExtractor> fn =
            std::make_shared<EitherIntBoolExtractor>(either);

        WorkManager::run(fn);
        ASSERT_EQ(fn->ret, result);
    }
}

struct EitherIntBoolEdgeCase
    : EasyCloneFn<EitherIntBoolEdgeCase, Bool, EitherIntBool> {
    using EasyCloneFn<EitherIntBoolEdgeCase, Bool, EitherIntBool>::EasyCloneFn;
    std::shared_ptr<Lazy<Bool>> y = nullptr;
    std::shared_ptr<Lazy<Bool>>
    body(std::shared_ptr<Lazy<EitherIntBool>> &either) override {
        WorkManager::await(either);
        EitherIntBool x = either->value();
        switch (x.tag) {
        case 0ULL: {
            std::shared_ptr<Lazy<Left::type>> i =
                std::make_shared<LazyConstant<Left::type>>(
                    reinterpret_cast<Left *>(&x.value)->value);
            std::shared_ptr<Lazy<Int>> z =
                std::make_shared<LazyConstant<Int>>(0);
            if (y == nullptr) {
                y = std::make_shared<Comparison_GT__BuiltIn>();
                dynamic_fn_cast<FnT<Bool, Int, Int>>(y)->args =
                    std::make_tuple(i, z);
                WorkManager::call(std::dynamic_pointer_cast<Fn>(y));
            }
            break;
        }
        case 1ULL: {
            std::shared_ptr<Lazy<Right::type>> b =
                std::make_shared<LazyConstant<Right::type>>(
                    reinterpret_cast<Right *>(&x.value)->value);
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

        std::shared_ptr<EitherIntBoolEdgeCase> fn =
            std::make_shared<EitherIntBoolEdgeCase>(either);

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
    std::shared_ptr<ListIntSum> call1 = nullptr;
    std::shared_ptr<Plus__BuiltIn> call2 = nullptr;
    std::shared_ptr<Lazy<Int>>
    body(std::shared_ptr<Lazy<ListInt>> &lazy_list) override {
        WorkManager::await(lazy_list);
        ListInt list = lazy_list->value();
        switch (list.tag) {
        case 0: {
            std::shared_ptr<Lazy<destroy_references_t<Cons::type>>> cons_lazy =
                std::make_shared<
                    LazyConstant<destroy_references_t<Cons::type>>>(
                    destroy_references(
                        reinterpret_cast<Cons *>(&list.value)->value));
            WorkManager::await(cons_lazy);
            destroy_references_t<Cons::type> cons = cons_lazy->value();
            Int head = std::get<0ULL>(cons);
            ListInt tail = std::get<1ULL>(cons);

            if (call1 == nullptr) {
                call1 = std::make_shared<ListIntSum>();
                call1->args = reference_all(tail);
                WorkManager::call(call1);
            }

            if (call2 == nullptr) {
                call2 = std::make_shared<Plus__BuiltIn>();
                call2->args =
                    std::tuple_cat(std::make_tuple(call1), reference_all(head));
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

    std::shared_ptr<ListIntSum> adder = std::make_shared<ListIntSum>(first);

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
    std::shared_ptr<Lazy<VariantT<Suc, Nil>>>
    body(std::shared_ptr<Lazy<VariantT<Suc, Nil>>> &nat_) override {
        WorkManager::await(nat_);
        VariantT<Suc, Nil> nat = nat_->value();
        switch (nat.tag) {
        case 0: {
            Suc::type s = reinterpret_cast<Suc *>(&nat.value)->value;
            VariantT<Suc, Nil> r = *s;
            return std::make_shared<LazyConstant<VariantT<Suc, Nil>>>(r);
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
    VariantT<Suc, Nil> n = {};
    n.tag = 1ULL;
    VariantT<Suc, Nil> *wrapped_n = new VariantT<Suc, Nil>{n};

    VariantT<Suc, Nil> inner = {};
    inner.tag = 0ULL;
    reinterpret_cast<Suc *>(&inner.value)->value = wrapped_n;
    Nat *wrapped_inner = new Nat(inner);

    VariantT<Suc, Nil> outer = {};

    outer.tag = 0ULL;
    reinterpret_cast<Suc *>(&outer.value)->value = wrapped_inner;

    std::shared_ptr<SimpleRecursiveTypeExample> fn =
        std::make_shared<SimpleRecursiveTypeExample>(outer);

    WorkManager::run(fn);
    ASSERT_EQ(fn->ret.tag, inner.tag);
    ASSERT_EQ(reinterpret_cast<Suc *>(&fn->ret.value)->value,
              reinterpret_cast<Suc *>(&inner.value)->value);
}

using F = TupleT<Lazy<FnT<Int, Int>> *>;
struct SelfRecursiveFn : Closure<SelfRecursiveFn, F, Int, Int> {
    using Closure<SelfRecursiveFn, F, Int, Int>::Closure;
    FnT<Int, Int> g = nullptr;
    std::shared_ptr<Lazy<Int>> body(std::shared_ptr<Lazy<Int>> &x) override {
        WorkManager::await(x);
        if (x->value() > 0) {
            auto lz = std::get<0>(this->env);
            auto f = lz->value();
            if (g == nullptr) {
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
    dynamic_cast<SelfRecursiveFn *>(f.get())->env =
        std::make_tuple(new LazyConstant<FnT<Int, Int>>{f});
    std::shared_ptr<Lazy<Int>> x = std::make_shared<LazyConstant<Int>>(5);
    f->args = std::make_tuple(x);

    WorkManager::run(f);
    ASSERT_EQ(f->ret, 0);
}

const std::vector<unsigned> cpu_counts = {1, 2, 3, 4};
INSTANTIATE_TEST_SUITE_P(FnCorrectnessTests, FnCorrectnessTest,
                         ::testing::ValuesIn(cpu_counts));
