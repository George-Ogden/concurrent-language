#pragma once

#include "system/thread_manager.tpp"
#include "work/status.hpp"

#include <gtest/gtest.h>

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

TEST(ExecutionStatusTransition, SingleRequiredTest) {
    Status status;
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_FALSE(status.queued());
    ASSERT_FALSE(status.required());
    status.enqueue();
    ASSERT_TRUE(status.require());
    ASSERT_TRUE(status.required());
    ASSERT_FALSE(status.require());
    ASSERT_TRUE(status.required());
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_TRUE(status.queued());
}

TEST(ExecutionStatusTransition, RequiredActiveTest) {
    Status status;
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_FALSE(status.queued());
    status.start_work();
    ASSERT_FALSE(status.required());
    ASSERT_TRUE(status.require());
    ASSERT_TRUE(status.required());
    ASSERT_FALSE(status.require());
    ASSERT_TRUE(status.required());
    ASSERT_EQ(status.execution_status(), Status::active);
    ASSERT_FALSE(status.queued());
}

TEST(ExecutionStatusTransition, RequiredDoneTest) {
    Status status;
    ASSERT_EQ(status.execution_status(), Status::available);
    ASSERT_FALSE(status.queued());
    status.start_work();
    status.finish_work();
    ASSERT_FALSE(status.required());
    ASSERT_FALSE(status.require());
    ASSERT_TRUE(status.required());
    ASSERT_FALSE(status.require());
    ASSERT_TRUE(status.required());
    ASSERT_EQ(status.execution_status(), Status::finished);
    ASSERT_FALSE(status.queued());
}
