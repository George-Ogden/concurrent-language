#pragma once

#include "data_structures/lock.tpp"
#include "fn/continuation.tpp"
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

class WorkStatusTest : public WorkTest {};

TEST_F(WorkStatusTest, QueuedStatus) {
    WorkRunner::shared_work_queue->clear();
    ASSERT_TRUE(work->status.acquire());
    auto n = work.use_count();
    WorkManager::enqueue(work);
    ASSERT_EQ(WorkRunner::shared_work_queue->size(), 1);
    ASSERT_EQ(work.use_count(), n);
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

struct WorkDependencyTest : WorkTest {};

TEST_F(WorkDependencyTest, Empty) { work->add_dependencies({}); }

struct WorkRunnerPriorityPropagationTest : WorkStatusTest {
  protected:
    void SetUp() override {
        WorkStatusTest::SetUp();
        WorkRunner::shared_work_queue->clear();
        WorkRunner::done_flag = false;
        WorkRunner::num_cpus = ThreadManager::available_concurrency();
    }
};

class PriorityChecker
    : public TypedClosureI<TupleT<WorkT *, bool, std::function<void()>>, Int> {
    using TypedClosureI<TupleT<WorkT *, bool, std::function<void()>>,
                        Int>::TypedClosureI;
    LazyT<Int> body() override {
        auto [work, priority, f] = env;
        f->value()();
        priority->lvalue() = (*work->value())->status.priority();
        return make_lazy<Int>(0);
    }

  public:
    static std::unique_ptr<TypedFnI<Int>> init(const ArgsT &args,
                                               const EnvT &env) {
        return std::make_unique<PriorityChecker>(args, env);
    }
    constexpr std::size_t lower_size_bound() const override { return 0; };
    constexpr std::size_t upper_size_bound() const override { return 0; };
    constexpr bool is_recursive() const override { return false; };
};

struct WorkRunnerCurrentWorkOverrider : WorkRunner {
    void set_current_work(WorkT work) { current_work = work; }
};

TEST_F(WorkRunnerPriorityPropagationTest, LowPriority) {
    WorkRunner::shared_work_queue->clear();
    WorkRunner::done_flag = false;
    WorkRunner::num_cpus = ThreadManager::available_concurrency();
    WorkT indirect_work, direct_work;
    LazyT<Int> v1, v2;
    typename PriorityChecker::EnvT env = std::make_tuple(
        make_lazy<WorkT *>(&indirect_work),
        std::make_shared<LazyConstant<bool>>(
            false), // bypass value cache when changing value
        make_lazy<std::function<void()>>(std::function<void()>([]() {})));

    FnT<Int> fn =
        std::make_shared<TypedClosureG<typename PriorityChecker::EnvT, Int>>(
            PriorityChecker::init, env);
    std::tie(indirect_work, v1) = Work::fn_call(fn);
    WorkManager::enqueue(indirect_work);

    std::tie(direct_work, v2) = Work::fn_call(Increment__BuiltIn_G, v1);

    ASSERT_FALSE(indirect_work->status.priority());
    ASSERT_FALSE(direct_work->status.priority());
    ASSERT_FALSE(indirect_work->status.done());
    ASSERT_FALSE(direct_work->status.done());

    static_cast<WorkRunnerCurrentWorkOverrider *>(WorkManager::runners[0].get())
        ->set_current_work(direct_work);
    direct_work->run();

    ASSERT_EQ(v2->value(), 1);
    ASSERT_FALSE(std::get<1>(env)->value());
    ASSERT_TRUE(direct_work->status.done());
    ASSERT_TRUE(indirect_work->status.done());
    ASSERT_FALSE(direct_work->status.priority());
    ASSERT_FALSE(indirect_work->status.priority());
}

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

class WorkContinuationTest : public WorkTest {};

TEST_F(WorkContinuationTest, IndirectContinuationAdded) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    result->add_continuation(Continuation{remaining, counter, valid});
    work->run();
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
    delete valid;
}

TEST_F(WorkContinuationTest, IndirectContinuationApplied) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    work->run();
    result->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
    delete valid;
}

TEST_F(WorkContinuationTest, DoneUnfinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{2};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    work->run();
    work->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    delete remaining;
    delete valid;
}

TEST_F(WorkContinuationTest, NotDoneUnfinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{2};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    work->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 2);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    work->run();
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    delete remaining;
    delete valid;
}

TEST_F(WorkContinuationTest, DoneFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    ASSERT_EQ(**valid, true);
    work->run();
    work->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
    delete valid;
}

TEST_F(WorkContinuationTest, NotDoneFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    work->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, true);
    work->run();
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
    delete valid;
}

TEST_F(WorkContinuationTest, DoneInvalidFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{false};
    ASSERT_EQ(**valid, false);
    work->run();
    work->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}

TEST_F(WorkContinuationTest, NotDoneInvalidFinishedContinuationBehaviour) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{false};
    work->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(remaining->load(std::memory_order_relaxed), 1);
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
    ASSERT_EQ(**valid, false);
    work->run();
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 1);
}
