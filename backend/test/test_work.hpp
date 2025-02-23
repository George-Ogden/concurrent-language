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

class StatusTest : public WorkTest {};

TEST_F(StatusTest, QueuedStatus) {
    ASSERT_FALSE(work->status.queued());
    WorkManager::enqueue(work);
    ASSERT_TRUE(work->status.queued());
    work->run();
    ASSERT_TRUE(work->status.done());
    WorkRunner::shared_work_queue->clear();
    work->status.dequeue();
    ASSERT_FALSE(work->status.queued());
    WorkManager::enqueue(work);
    ASSERT_EQ(WorkRunner::shared_work_queue->size(), 0);
    ASSERT_FALSE(work->status.queued());
    ASSERT_TRUE(work->status.done());
}

class RunningStatusChecker
    : public TypedClosureI<TupleT<WorkT *, Status::ExecutionStatus>, TupleT<>> {
    using TypedClosureI<TupleT<WorkT *, Status::ExecutionStatus>,
                        TupleT<>>::TypedClosureI;
    LazyT<TupleT<>> body() override {
        auto [work, running_status] = env;
        running_status->lvalue() = (*work->value())->status.execution_status();
        return std::make_tuple();
    }

  public:
    static std::unique_ptr<TypedFnI<TupleT<>>> init(const ArgsT &args,
                                                    std::shared_ptr<EnvT> env) {
        return std::make_unique<RunningStatusChecker>(args, *env);
    }
};

TEST_F(StatusTest, RunningStatus) {
    WorkT work;
    auto env = std::make_tuple(make_lazy<WorkT *>(&work),
                               make_lazy<Status::ExecutionStatus>());

    LazyT<TupleT<>> v;
    std::tie(work, v) = Work::fn_call(
        TypedClosureG<typename RunningStatusChecker::EnvT, TupleT<>>{
            RunningStatusChecker::init, env});

    ASSERT_EQ(work->status.execution_status(), Status::available);
    work->run();
    ASSERT_EQ(std::get<1>(env)->value(), Status::active);
    ASSERT_EQ(work->status.execution_status(), Status::finished);
}

TEST_F(StatusTest, RunningFromQueueStatus) {
    WorkT work;
    auto env = std::make_tuple(make_lazy<WorkT *>(&work),
                               make_lazy<Status::ExecutionStatus>());

    LazyT<TupleT<>> v;
    std::tie(work, v) = Work::fn_call(
        TypedClosureG<typename RunningStatusChecker::EnvT, TupleT<>>{
            RunningStatusChecker::init, env});

    WorkManager::enqueue(work);
    ASSERT_TRUE(work->status.queued());
    ASSERT_EQ(work->status.execution_status(), Status::available);
    work->run();
    ASSERT_EQ(std::get<1>(env)->value(), Status::active);
    ASSERT_TRUE(work->status.done());
}

class PairFn : public TypedClosureI<Empty, TupleT<Int, Int>, Int, Int> {
    using TypedClosureI<Empty, TupleT<Int, Int>, Int, Int>::TypedClosureI;
    LazyT<TupleT<Int, Int>> body(LazyT<Int> &x, LazyT<Int> &y) override {
        return std::make_tuple(x, y);
    }

  public:
    static std::unique_ptr<TypedFnI<TupleT<Int, Int>, Int, Int>>
    init(const ArgsT &args, std::shared_ptr<void>) {
        return std::make_unique<PairFn>(args);
    }
};

TEST(TupleWorkTest, CorrectValue) {
    std::shared_ptr<Work> work;
    LazyT<TupleT<Int, Int>> results;
    TypedFnG<TupleT<Int, Int>, Int, Int> pair_fn =
        TypedClosureG<Empty, TupleT<Int, Int>, Int, Int>{PairFn::init};
    std::tie(work, results) =
        Work::fn_call(pair_fn, make_lazy<Int>(4), make_lazy<Int>(-4));
    work->run();
    ASSERT_TRUE(work->done());
    ASSERT_EQ(std::get<0>(results)->value(), 4);
    ASSERT_EQ(std::get<1>(results)->value(), -4);
};

class WorkContinuationTest : public WorkTest {};

TEST_F(WorkTest, IndirectContinuationAdded) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    result->add_continuation(Continuation{remaining, counter, valid});
    work->run();
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
}

TEST_F(WorkTest, IndirectContinuationApplied) {
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{1};
    std::atomic<unsigned> counter{1};
    Locked<bool> *valid = new Locked<bool>{true};
    work->run();
    result->add_continuation(Continuation{remaining, counter, valid});
    ASSERT_EQ(counter.load(std::memory_order_relaxed), 2);
    ASSERT_EQ(**valid, false);
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
