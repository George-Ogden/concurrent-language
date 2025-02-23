#pragma once

#include "data_structures/lock.tpp"
#include "fn/continuation.tpp"
#include "fn/fn.tpp"
#include "fn/types.hpp"
#include "lazy/lazy.tpp"
#include "lazy/types.hpp"
#include "system/thread_manager.tpp"
#include "system/work_manager.tpp"
#include "types/compound.hpp"
#include "work/runner.tpp"
#include "work/work.tpp"

#include "test/inc.hpp"

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
        std::tie(work, result) = Work::fn_call(inc_fn, make_lazy<Int>(4));
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

LazyT<TupleT<>> running_status_checker(
    std::shared_ptr<
        std::tuple<std::shared_ptr<Work> *, Status::ExecutionStatus *>>
        env) {
    auto [work, running_status] = *env;
    *running_status = (*work)->status.execution_status();
    return std::make_tuple();
};

TEST_F(StatusTest, RunningStatus) {
    WorkT work;
    Status::ExecutionStatus running_status;

    TypedClosure<std::tuple<std::shared_ptr<Work> *, Status::ExecutionStatus *>,
                 LazyT<TupleT<>>>
        running_status_checker_fn(running_status_checker,
                                  std::make_tuple(&work, &running_status));

    LazyT<TupleT<>> v;
    std::tie(work, v) = Work::fn_call(running_status_checker_fn);

    ASSERT_EQ(work->status.execution_status(), Status::available);
    work->run();
    ASSERT_EQ(running_status, Status::active);
    ASSERT_EQ(work->status.execution_status(), Status::finished);
}

TEST_F(StatusTest, RunningFromQueueStatus) {
    WorkT work;
    Status::ExecutionStatus running_status;

    TypedClosure<std::tuple<std::shared_ptr<Work> *, Status::ExecutionStatus *>,
                 LazyT<TupleT<>>>
        running_status_checker_fn(running_status_checker,
                                  std::make_tuple(&work, &running_status));

    LazyT<TupleT<>> v;
    std::tie(work, v) = Work::fn_call(running_status_checker_fn);

    WorkManager::enqueue(work);
    ASSERT_TRUE(work->status.queued());
    work->run();
    ASSERT_EQ(running_status, Status::active);
    ASSERT_TRUE(work->status.done());
}

LazyT<TupleT<Int, Int>> pair(LazyT<Int> x, LazyT<Int> y,
                             std::shared_ptr<void> env = nullptr) {
    return std::make_tuple(x, y);
}

FnT<TupleT<Int, Int>, Int, Int> pair_fn{pair};

TEST(TupleWorkTest, CorrectValue) {
    std::shared_ptr<Work> work;
    LazyT<TupleT<Int, Int>> results;
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
