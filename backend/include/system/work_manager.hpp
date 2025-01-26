#pragma once

#include "fn/fn.hpp"
#include "system/thread_manager.hpp"
#include "system/work_manager_pre.hpp"

#include <atomic>
#include <memory>
#include <optional>
#include <utility>
#include <vector>

void WorkManager::run(std::shared_ptr<Fn> fn) {
    std::atomic<std::shared_ptr<Fn>> ref{fn};
    ThreadManager::RunConfig config{ThreadManager::available_concurrency(),
                                    false};
    WorkManager::queue->clear();
    WorkManager::counters = std::vector<std::atomic<unsigned>>(
        ThreadManager::available_concurrency());
    ThreadManager::run_multithreaded(main, &ref, config);
    WorkManager::queue->clear();
}

void WorkManager::call(std::shared_ptr<Fn> fn) {
    WorkManager::queue.acquire();
    WorkManager::queue->push_back(fn);
    WorkManager::queue.release();
}

std::monostate WorkManager::main(std::atomic<std::shared_ptr<Fn>> *ref) {
    {
        std::shared_ptr<Fn> fn =
            ref->exchange(nullptr, std::memory_order_relaxed);
        if (fn != nullptr) {
            fn->run();
            call(std::make_shared<FinishWork>());
        }
    }
    while (1) {

        std::shared_ptr<Fn> fn = get_work();
        if (fn == nullptr) {
            sleep(1us);
            continue;
        }
        if (dynamic_cast<FinishWork *>(fn.get()) != nullptr) {
            call(fn);
            break;
        }
        fn->run();
    }
    return std::monostate{};
}

std::shared_ptr<Fn> WorkManager::get_work() {
    WorkManager::queue.acquire();
    if (WorkManager::queue->empty()) {
        WorkManager::queue.release();
        return nullptr;
    }
    std::shared_ptr<Fn> fn = WorkManager::queue->front();
    WorkManager::queue->pop_front();
    WorkManager::queue.release();
    return fn;
}

template <typename... Vs> void WorkManager::await(Vs &...vs) {
    unsigned n = sizeof...(vs);
    if (n == 0) {
        return;
    }
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{n};
    std::atomic<unsigned> &counter = counters[ThreadManager::get_id()];
    Locked<bool> *valid = new Locked<bool>{true};
    Continuation c{remaining, counter, *valid};
    (vs->add_continuation(c), ...);
    if (all_done(vs...)) {
        counter.fetch_sub(1, std::memory_order_relaxed);
        return;
    }
    while (true) {
        std::shared_ptr<Fn> fn = get_work();
        try {
            if (counter.load(std::memory_order_relaxed) > 0) {
                throw stack_inversion{};
            }
            if (fn != nullptr) {
                fn->run();
            }
        } catch (stack_inversion &e) {
            if (fn != nullptr && !fn->done()) {
                call(fn);
            }
            valid->acquire();
            bool was_valid = **valid;
            **valid = false;
            valid->release();
            if (all_done(vs...)) {
                counter.fetch_sub(1 - was_valid, std::memory_order_relaxed);
                return;
            } else {
                throw;
            }
        }
    }
};
