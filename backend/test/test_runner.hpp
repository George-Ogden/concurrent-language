#pragma once

#include <gtest/gtest.h>

#include "fn/fn_gen.tpp"
#include "fn/fn_inst.tpp"
#include "fn/operators.hpp"
#include "lazy/lazy.tpp"
#include "system/thread_manager.tpp"
#include "system/work_manager.tpp"
#include "types/builtin.hpp"
#include "work/runner.tpp"
#include "work/work.tpp"

#include <memory>

struct PublicWorkRunner : WorkRunner {
    using WorkRunner::WorkRunner;
    std::vector<WorkT> get_small_works() const { return small_works; }
    std::vector<WorkT> get_large_works() const { return large_works; }
    bool enqueue(const WorkT &work) { return WorkRunner::enqueue(work); }
};

class RunnerTest : public ::testing::Test {
  protected:
    std::unique_ptr<PublicWorkRunner> runner;
    void SetUp() override {
        ThreadManager::override_concurrency(1);
        ThreadManager::register_self(0);
        runner = std::make_unique<PublicWorkRunner>(0);
    }
    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

class LargeWork : public TypedClosureI<Empty, Int> {
    using TypedClosureI<Empty, Int>::TypedClosureI;
    LazyT<Int> body() override { return make_lazy<Int>(0); }

  public:
    static std::unique_ptr<TypedFnI<Int>> init(const ArgsT &args) {
        return std::make_unique<LargeWork>(args);
    }
    constexpr std::size_t lower_size_bound() const override { return 5000; };
    constexpr std::size_t upper_size_bound() const override { return 5000; };
    constexpr bool is_recursive() const override { return true; };
};

TEST_F(RunnerTest, RunnerEnqueueSmallWork) {
    auto [small_work, result] =
        Work::fn_call(Increment__BuiltIn_G, make_lazy<Int>(4));
    ASSERT_EQ(runner->get_small_works(), std::vector<WorkT>{});
    ASSERT_EQ(runner->get_large_works(), std::vector<WorkT>{});
    ASSERT_FALSE(small_work->queued());

    // Add work + update status.
    ASSERT_TRUE(runner->enqueue(small_work));
    ASSERT_TRUE(small_work->queued());
    ASSERT_EQ(runner->get_small_works(), std::vector<WorkT>{small_work});
    ASSERT_EQ(runner->get_large_works(), std::vector<WorkT>{});

    // No change.
    ASSERT_FALSE(runner->enqueue(small_work));
    ASSERT_TRUE(small_work->queued());
    ASSERT_EQ(runner->get_small_works(), std::vector<WorkT>{small_work});
    ASSERT_EQ(runner->get_large_works(), std::vector<WorkT>{});
}

TEST_F(RunnerTest, RunnerEnqueueBigWork) {
    FnT<Int> large_fn =
        std::make_shared<TypedClosureG<Empty, Int>>(LargeWork::init);
    auto [large_work, result] = Work::fn_call(large_fn);
    ASSERT_EQ(runner->get_small_works(), std::vector<WorkT>{});
    ASSERT_EQ(runner->get_large_works(), std::vector<WorkT>{});
    ASSERT_FALSE(large_work->queued());

    // Add work + update status.
    ASSERT_TRUE(runner->enqueue(large_work));
    ASSERT_TRUE(large_work->queued());
    ASSERT_EQ(runner->get_small_works(), std::vector<WorkT>{});
    ASSERT_EQ(runner->get_large_works(), std::vector<WorkT>{large_work});

    // No change.
    ASSERT_FALSE(runner->enqueue(large_work));
    ASSERT_TRUE(large_work->queued());
    ASSERT_EQ(runner->get_small_works(), std::vector<WorkT>{});
    ASSERT_EQ(runner->get_large_works(), std::vector<WorkT>{large_work});
}
