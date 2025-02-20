#pragma once

#include "system/thread_manager.tpp"
#include "work/status.hpp"

#include <gtest/gtest.h>

TEST(AtomicSharedEnumTest, BitFlip) {
    AtomicSharedEnum<1, 2, 1> byte_array;
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_FALSE(byte_array.flip<0>());
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    ASSERT_FALSE(byte_array.flip<2>());
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 1);
    ASSERT_TRUE(byte_array.flip<2>());
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
}

TEST(AtomicSharedEnumTest, CompareExchange) {
    AtomicSharedEnum<2, 1> byte_array;
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_TRUE(byte_array.compare_exchange<0>(0, 3));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_FALSE(byte_array.compare_exchange<0>(2, 1));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_TRUE(byte_array.compare_exchange<1>(0, 1));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_FALSE(byte_array.compare_exchange<1>(0, 1));
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_TRUE(byte_array.compare_exchange<0>(3, 2));
    ASSERT_EQ(byte_array.load<0>(), 2);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_TRUE(byte_array.compare_exchange<1>(1, 0));
    ASSERT_EQ(byte_array.load<0>(), 2);
    ASSERT_EQ(byte_array.load<1>(), 0);
}

TEST(AtomicSharedEnumTest, Exchange) {
    AtomicSharedEnum<2, 1> byte_array;
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.exchange<0>(3), 0);
    ASSERT_EQ(byte_array.load<0>(), 3);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.exchange<0>(1), 3);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.exchange<1>(1), 0);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.exchange<1>(1), 1);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.exchange<0>(2), 1);
    ASSERT_EQ(byte_array.load<0>(), 2);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.exchange<1>(0), 1);
    ASSERT_EQ(byte_array.load<0>(), 2);
    ASSERT_EQ(byte_array.load<1>(), 0);
}

TEST(AtomicSharedEnumTest, Store) {
    AtomicSharedEnum<1, 2, 1> byte_array;
    ASSERT_EQ(byte_array.load<0>(), 0);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    byte_array.store<0>(1);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 0);
    ASSERT_EQ(byte_array.load<2>(), 0);
    byte_array.store<1>(3);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 3);
    ASSERT_EQ(byte_array.load<2>(), 0);
    byte_array.store<1>(2);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 2);
    ASSERT_EQ(byte_array.load<2>(), 0);
    byte_array.store<2>(1);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 2);
    ASSERT_EQ(byte_array.load<2>(), 1);
    byte_array.store<1>(1);
    ASSERT_EQ(byte_array.load<0>(), 1);
    ASSERT_EQ(byte_array.load<1>(), 1);
    ASSERT_EQ(byte_array.load<2>(), 1);
}

TEST(ExecutionStatusTransition, UnqueuedExecutionTest) {
    Status status;
    ASSERT_FALSE(status.queued());
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_FALSE(status.cancel_work());
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_TRUE(status.start_work());
    ASSERT_EQ(status.execution_status(), Status::active);
    ASSERT_FALSE(status.done());
    ASSERT_FALSE(status.start_work());
    ASSERT_EQ(status.execution_status(), Status::active);
    ASSERT_TRUE(status.cancel_work());
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_TRUE(status.start_work());
    status.finish_work();
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_TRUE(status.done());
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_FALSE(status.start_work());
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_FALSE(status.cancel_work());
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_FALSE(status.queued());
}

TEST(ExecutionStatusTransition, QueuedExecutionTest) {
    Status status;
    ASSERT_FALSE(status.queued());
    ASSERT_TRUE(status.enqueue());
    ASSERT_TRUE(status.queued());
    ASSERT_FALSE(status.done());
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_FALSE(status.cancel_work());
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_TRUE(status.start_work());
    ASSERT_EQ(status.execution_status(), Status::active);
    ASSERT_FALSE(status.done());
    ASSERT_FALSE(status.start_work());
    ASSERT_EQ(status.execution_status(), Status::active);
    ASSERT_TRUE(status.cancel_work());
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_TRUE(status.start_work());
    status.finish_work();
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_TRUE(status.done());
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_FALSE(status.start_work());
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_FALSE(status.cancel_work());
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_TRUE(status.queued());
}

TEST(ExecutionStatusTransition, AvailableQueueingTest) {
    Status status;
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_FALSE(status.queued());
    ASSERT_TRUE(status.enqueue());
    ASSERT_TRUE(status.queued());
    ASSERT_FALSE(status.enqueue());
    ASSERT_TRUE(status.queued());
    ASSERT_TRUE(status.dequeue());
    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.dequeue());
    ASSERT_FALSE(status.queued());
    ASSERT_EQ(status.execution_status(), Status::available);
}

TEST(ExecutionStatusTransition, ActiveQueueingTest) {
    Status status;
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_FALSE(status.queued());
    status.start_work();
    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.enqueue());
    ASSERT_FALSE(status.queued());
    status.cancel_work();
    ASSERT_FALSE(status.queued());
    ASSERT_TRUE(status.enqueue());
    ASSERT_TRUE(status.queued());
    status.start_work();
    ASSERT_FALSE(status.dequeue());
    ASSERT_FALSE(status.queued());
    ASSERT_EQ(status.execution_status(), Status::active);
}

TEST(ExecutionStatusTransition, DoneEnqueueTest) {
    Status status;
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_FALSE(status.queued());
    status.start_work();
    status.finish_work();
    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.enqueue());
    ASSERT_FALSE(status.queued());
    ASSERT_EQ(status.execution_status(), Status::finished);
}

TEST(ExecutionStatusTransition, DoneDequeueTest) {
    Status status;
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_FALSE(status.queued());
    ASSERT_TRUE(status.enqueue());
    ASSERT_TRUE(status.queued());
    status.start_work();
    status.finish_work();
    ASSERT_TRUE(status.queued());
    ASSERT_FALSE(status.dequeue());
    ASSERT_FALSE(status.queued());
    ASSERT_EQ(status.execution_status(), Status::finished);
}
