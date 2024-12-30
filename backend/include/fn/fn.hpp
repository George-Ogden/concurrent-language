#pragma once

#include "data_structures/lock.hpp"
#include "time/sleep.hpp"

#include <atomic>
#include <chrono>
#include <cstdint>
#include <deque>
#include <tuple>
#include <type_traits>
#include <vector>

using namespace std::literals::chrono_literals;

class Fn {
    friend class Workers;
    static inline ExchangeLock lock;
    static inline std::deque<Fn *> queue;

  protected:
    virtual void body() = 0;

  public:
    virtual ~Fn() = default;
    void run() {
        body();
        for (auto &cont : conts) {
            if (cont->deps.fetch_sub(1, std::memory_order_relaxed) == 1) {
                cont->call();
            }
        }
    }
    virtual void call() {
        lock.acquire();
        queue.push_back(this);
        lock.release();
    }
    std::vector<Fn *> conts;
    std::atomic<uint32_t> deps;
};

template <typename Ret, typename... Args> struct ParametricFn : public Fn {
    using ArgsT = std::tuple<std::add_pointer_t<Args>...>;
    ArgsT args;
    Ret *ret;
};

class FinishWork : public Fn {
    void body() override {}
};
