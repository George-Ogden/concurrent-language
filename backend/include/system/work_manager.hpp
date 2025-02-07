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
            fn->await_all();
            call(std::make_shared<FinishWork>());
        } else {
            while (1) {
                fn = get_work();
                if (fn == nullptr) {
                    sleep(1us);
                    continue;
                }
                if (dynamic_cast<FinishWork *>(fn.get()) != nullptr) {
                    call(fn);
                    break;
                }
                try {
                    fn->run();
                } catch (finished &e) {
                    break;
                }
            }
        }
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

template <typename T> constexpr auto filter_awaitable(T &v) {
    return std::tuple<std::decay_t<T>>(v);
}

template <typename... Ts>
constexpr auto filter_awaitable(std::tuple<Ts...> &v) {
    return std::tuple<>{};
}

template <typename... Vs> void WorkManager::await(Vs &...vs) {
    std::apply([&](auto &&...ts) { await_restricted(ts...); },
               std::tuple_cat(filter_awaitable(vs)...));
}

template <typename T> void await_variants(T &v) {}

template <typename... Ts>
void await_variants(std::shared_ptr<Lazy<VariantT<Ts...>>> &l) {
    auto v = l->value();
    std::size_t idx = v.tag;
    using AwaitFn = void (*)(std::aligned_union_t<0, Ts...> &);

    static constexpr AwaitFn waiters[sizeof...(Ts)] = {[](auto &storage) {
        WorkManager::await_all(
            std::launder(reinterpret_cast<Ts *>(&storage))->value);
    }...};

    waiters[idx](v.value);
}

template <typename... Vs> void WorkManager::await_all(Vs &...vs) {
    if constexpr (sizeof...(vs) != 0) {
        auto flat_types = flatten(std::make_tuple(vs...));
        std::apply([&](auto &&...ts) { await_restricted(ts...); }, flat_types);
        std::apply([&](auto &&...ts) { (await_variants(ts), ...); },
                   flat_types);
    }
}

template <typename... Vs> void WorkManager::await_restricted(Vs &...vs) {
    unsigned n = sizeof...(vs);
    if (n == 0) {
        return;
    }
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{n};
    std::atomic<unsigned> &counter = counters[ThreadManager::get_id()];
    Locked<bool> *valid = new Locked<bool>{true};
    Continuation c{remaining, counter, valid};
    (vs->add_continuation(c), ...);
    if (all_done(vs...)) {
        delete valid;
        if (counter.fetch_sub(1, std::memory_order_relaxed) == 1) {
            return;
        } else {
            throw stack_inversion{};
        }
    }
    while (true) {
        std::shared_ptr<Fn> fn = get_work();
        if (dynamic_cast<FinishWork *>(fn.get()) != nullptr) {
            call(fn);
            throw finished{};
        }
        try {
            if (counter.load(std::memory_order_relaxed) > 0) {
                throw stack_inversion{};
            }
            if (fn != nullptr) {
                fn->run();
            }
        } catch (stack_inversion &e) {
            valid->acquire();
            bool was_valid = **valid;
            **valid = false;
            valid->release();
            if (fn != nullptr && !fn->done()) {
                call(fn);
            }
            if (!was_valid) {
                delete valid;
            }
            counter.fetch_sub(1 - was_valid, std::memory_order_relaxed);

            if (!was_valid && counter.load(std::memory_order_relaxed) == 0) {
                while (!all_done(vs...)) {
                }
                return;
            }
            throw;
        }
    }
};
