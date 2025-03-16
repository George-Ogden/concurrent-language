#pragma once

#include "data_structures/lock.tpp"
#include "fn/fn_gen.tpp"
#include "fn/operators.hpp"
#include "fn/types.hpp"
#include "lazy/lazy.tpp"
#include "lazy/types.hpp"
#include "system/thread_manager.tpp"
#include "system/work_manager.tpp"
#include "types/compound.hpp"
#include "work/runner.tpp"
#include "work/work.tpp"

#include <gtest/gtest.h>

#include <atomic>
#include <functional>
#include <memory>
#include <tuple>
#include <vector>

class WorkTest : public ::testing::Test {
  protected:
    std::shared_ptr<Work> work;
    LazyT<Int> result;
    void SetUp() override {
        ThreadManager::override_concurrency(1);
        ThreadManager::register_self(0);
        WorkManager::runners.clear();
        WorkManager::runners.emplace_back(std::make_unique<WorkRunner>(0));
        std::tie(work, result) =
            Work::fn_call(Increment__BuiltIn_G, make_lazy<Int>(4));
    }
    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

TEST_F(WorkTest, DoneLater) {
    ASSERT_FALSE(work->done());
    ASSERT_FALSE(result->done());
    work->run();
    ASSERT_TRUE(result->done());
    ASSERT_TRUE(work->done());
}

TEST_F(WorkTest, CorrectValue) {
    work->run();
    ASSERT_EQ(result->value(), 5);
}

struct LazyWorkTest : WorkTest {};

TEST_F(LazyWorkTest, GetLazyConstantWork) {
    std::vector<WorkT> works;
    auto y = LazyConstant<Int>(10);
    y.get_work(works);
    ASSERT_EQ(works, std::vector<WorkT>{});
}

TEST_F(LazyWorkTest, GetLazyRequiredWork) {
    std::vector<WorkT> works;
    result->get_work(works);
    ASSERT_EQ(works, std::vector<WorkT>{work});
}

TEST_F(LazyWorkTest, GetLazyDoneWork) {
    std::vector<WorkT> works;
    work->run();
    result->get_work(works);
    ASSERT_EQ(works, std::vector<WorkT>{});
}

struct SmallFn : TypedClosureI<Empty, Int> {
    LazyT<Int> body() { return make_lazy<Int>(0); }
    constexpr std::size_t lower_size_bound() const { return 10; }
    constexpr std::size_t upper_size_bound() const { return 20; }
    constexpr bool is_recursive() const { return false; }
};

struct LargeFn : TypedClosureI<Empty, Int> {
    LazyT<Int> body() { return make_lazy<Int>(0); }
    constexpr std::size_t lower_size_bound() const { return 50; }
    constexpr std::size_t upper_size_bound() const { return 100; }
    constexpr bool is_recursive() const { return false; }
};

struct ReferenceWork : TypedWork<Int> {
    explicit ReferenceWork(std::unique_ptr<TypedFnI<Int>> fn) {
        this->fn = std::move(fn);
    };
};

class PairFn : public TypedClosureI<Empty, TupleT<Int, Int>, Int, Int> {
    using TypedClosureI<Empty, TupleT<Int, Int>, Int, Int>::TypedClosureI;
    LazyT<TupleT<Int, Int>> body(LazyT<Int> &x, LazyT<Int> &y) override {
        return std::make_tuple(x, y);
    }

  public:
    static std::unique_ptr<TypedFnI<TupleT<Int, Int>, Int, Int>>
    init(const ArgsT &args) {
        return std::make_unique<PairFn>(args);
    }
    constexpr std::size_t lower_size_bound() const override { return 10; };
    constexpr std::size_t upper_size_bound() const override { return 10; };
    constexpr bool is_recursive() const override { return false; };
};

TEST(TupleWorkTest, CorrectValue) {
    std::shared_ptr<Work> work;
    LazyT<TupleT<Int, Int>> results;
    FnT<TupleT<Int, Int>, Int, Int> pair_fn =
        std::make_shared<TypedClosureG<Empty, TupleT<Int, Int>, Int, Int>>(
            PairFn::init);
    std::tie(work, results) =
        Work::fn_call(pair_fn, make_lazy<Int>(4), make_lazy<Int>(-4));
    work->run();
    ASSERT_TRUE(work->done());
    ASSERT_EQ(std::get<0>(results)->value(), 4);
    ASSERT_EQ(std::get<1>(results)->value(), -4);
};
