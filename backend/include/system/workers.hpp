#pragma once

#include "fn/fn.hpp"
#include "system/thread_manager.hpp"

#include <atomic>
#include <utility>

struct Workers {
    static void run(Fn *fn) {
        Fn *finish = new FinishWork{};
        finish->deps = 1;
        fn->conts.push_back(finish);
        std::atomic<Fn *> ref{fn};
        ThreadManager::RunConfig config{ThreadManager::available_concurrency(),
                                        false};
        ThreadManager::run_multithreaded(main, &ref, config);
        Fn::queue.pop_front();
    }

  protected:
    static std::monostate main(std::atomic<Fn *> *ref) {
        {
            Fn *fn = ref->exchange(nullptr, std::memory_order_relaxed);
            if (fn != nullptr) {
                fn->call();
            }
        }
        while (1) {
            Fn::lock.acquire();
            if (Fn::queue.empty()) {
                Fn::lock.release();
                sleep(1us);
                continue;
            }
            Fn *fn = Fn::queue.front();
            Fn::queue.pop_front();
            if (dynamic_cast<FinishWork *>(fn) != nullptr) {
                delete fn;
                Fn::queue.push_back(new FinishWork{});
                Fn::lock.release();
                break;
            }
            Fn::lock.release();
            fn->run();
            delete fn;
        }
        return std::monostate{};
    }
};
