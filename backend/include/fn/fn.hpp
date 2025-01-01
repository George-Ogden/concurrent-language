#pragma once

#include "data_structures/lazy.hpp"
#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
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
    virtual void run() = 0;
    virtual bool done() const = 0;

  public:
    virtual ~Fn() = default;
    virtual void call() {
        lock.acquire();
        queue.push_back(this);
        lock.release();
    }
};

template <typename Ret, typename... Args>
struct ParametricFn : public Fn, Lazy<Ret> {
    using ArgsT = std::tuple<std::add_pointer_t<Lazy<std::decay_t<Args>>>...>;
    using R = std::decay_t<Ret>;
    std::atomic<bool> done_flag{false};
    Locked<std::vector<Continuation>> continuations;
    ArgsT args;
    R ret;
    virtual R body(std::add_const_t<Args>...) = 0;
    void run() override {
        // await args;
        ret = std::apply([this](auto... T) { return body(T...); },
                         expand_values(args));
        done_flag.store(true, std::memory_order_release);
        continuations.acquire();
        for (const Continuation &c : *continuations) {
            update_continuation(c);
        }
        continuations.release();
    }
    template <typename Tuple> auto expand_values(Tuple &&t) const {
        return []<std::size_t... I>(auto &&t, std::index_sequence<I...>) {
            return std::make_tuple(
                (std::get<I>(std::forward<decltype(t)>(t))->value())...);
        }
        (std::forward<Tuple>(t),
         std::make_index_sequence<
             std::tuple_size_v<std::remove_reference_t<Tuple>>>{});
    }
    bool done() const override {
        return done_flag.load(std::memory_order_relaxed);
    }
    R value() override { return ret; }
    void add_continuation(Continuation c) override {
        continuations.acquire();
        if (done()) {
            continuations.release();
            update_continuation(c);
        } else {
            continuations->push_back(c);
            continuations.release();
        }
    }
    void update_continuation(Continuation c) {
        if (c.remaining.fetch_sub(1, std::memory_order_relaxed) == 1) {
            c.counter.fetch_add(1, std::memory_order_relaxed);
        }
    }
};

class FinishWork : public Fn {
    void run() override{};
    bool done() const override { return true; };
};
