#pragma once

#include "work/runner.tpp"
#include "work/work.tpp"

#include <gtest/gtest.h>

#include <atomic>
#include <thread>

struct PublicWorkRunner : WorkRunner {
    using WorkRunner::any_requests;
    using WorkRunner::request_work;
    using WorkRunner::respond;
    using WorkRunner::WorkRunner;
};

struct RunnerTest : public ::testing::Test {
  protected:
    void SetUp() override {
        ThreadManager::override_concurrency(2);
        WorkManager::runners.clear();
        WorkManager::runners.emplace_back(
            std::make_unique<PublicWorkRunner>(0));
        WorkManager::runners.emplace_back(
            std::make_unique<PublicWorkRunner>(1));
        WorkRunner::work_request_queue = CyclicQueue<std::atomic<WorkT> *>{2};
    }
    void TearDown() override { ThreadManager::reset_concurrency_override(); }
};

struct DummyWork : Work {
    void run() override {}
    void await_all() override {}
    std::size_t size() const override { return 0; }
};

TEST_F(RunnerTest, RequestResponseTest) {
    bool u1, u2, u3;
    bool r1, r2;
    WorkT w;
    PublicWorkRunner &runner_0 = *reinterpret_cast<PublicWorkRunner *>(
                         &*WorkManager::runners[0]),
                     &runner_1 = *reinterpret_cast<PublicWorkRunner *>(
                         &*WorkManager::runners[1]);
    std::atomic<unsigned> t{0};
    std::thread t1([&]() {
        ThreadManager::register_self(0);
        u1 = runner_0.any_requests();
        t.store(1);
        while (WorkRunner::work_request_queue.empty()) {
        }
        u2 = runner_0.any_requests();
        WorkT work = std::make_shared<DummyWork>();
        r1 = runner_0.respond(work);
        u3 = runner_0.any_requests();
        r2 = runner_0.respond(work);
    });
    std::thread t2([&]() {
        ThreadManager::register_self(1);
        while (t.load() != 1) {
        };
        w = runner_1.request_work();
    });

    t1.join();
    t2.join();

    ASSERT_EQ(u1, false);
    ASSERT_EQ(u2, true);
    ASSERT_EQ(u3, false);

    ASSERT_NE(w, nullptr);
    ASSERT_NE(std::dynamic_pointer_cast<DummyWork>(w), nullptr);

    ASSERT_EQ(r1, true);
    ASSERT_EQ(r2, false);
}
