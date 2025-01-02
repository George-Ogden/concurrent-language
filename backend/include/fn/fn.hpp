#pragma once

#include "data_structures/lazy.hpp"
#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
#include "system/work_manager_pre.hpp"
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
    friend struct WorkManager;

  protected:
    virtual void run() = 0;
    virtual bool done() const = 0;

  public:
    virtual ~Fn() = default;
    virtual void call() {
        WorkManager::queue.acquire();
        WorkManager::queue->push_back(this);
        WorkManager::queue.release();
    }
    template <typename Tuple> auto static expand_values(Tuple &&t) {
        return []<std::size_t... I>(auto &&t, std::index_sequence<I...>) {
            return std::make_tuple(
                (std::get<I>(std::forward<decltype(t)>(t))->value())...);
        }
        (std::forward<Tuple>(t),
         std::make_index_sequence<
             std::tuple_size_v<std::remove_reference_t<Tuple>>>{});
    }
    template <typename... Ts> auto static reference_all(Ts... args) {
        return std::make_tuple(new LazyConstant<std::decay_t<Ts>>(args)...);
    }
    template <typename T, typename... Ts>
    static void initialize(T *&fn, Ts &&...args) {
        if (fn == nullptr) {
            fn = new T(std::forward<Ts>(args)...);
        }
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
    ParametricFn() = default;
    template <typename = std::enable_if<(sizeof...(Args) > 0)>>
    explicit ParametricFn(
        std::add_const_t<std::add_lvalue_reference_t<Args>>... args)
        : args(reference_all(args...)) {}
    virtual R body(std::add_const_t<
                   std::add_lvalue_reference_t<std::decay_t<Args>>>...) = 0;
    void run() override {
        std::apply(
            WorkManager::await<std::add_pointer_t<Lazy<std::decay_t<Args>>>...>,
            args);
        ret = std::apply([this](auto &&...t) { return body(t...); },
                         expand_values(args));
        done_flag.store(true, std::memory_order_release);
        continuations.acquire();
        for (const Continuation &c : *continuations) {
            Lazy<Ret>::update_continuation(c);
        }
        continuations.release();
    }
    bool done() const override {
        return done_flag.load(std::memory_order_relaxed);
    }
    R value() override { return ret; }
    void add_continuation(Continuation c) override {
        continuations.acquire();
        if (done()) {
            continuations.release();
            Lazy<Ret>::update_continuation(c);
        } else {
            continuations->push_back(c);
            continuations.release();
        }
    }
};

class FinishWork : public Fn {
    void run() override{};
    bool done() const override { return true; };
};

template <typename T> struct BlockFn : public ParametricFn<T> {
    std::function<T()> body_fn;
    T body() override { return body_fn(); }
    bool done() const override { return ParametricFn<T>::done(); }
    explicit BlockFn(std::function<T()> &&f) : body_fn(std::move(f)){};
};

template <typename F> auto Block(F &&f) {
    using T = std::invoke_result_t<F>;
    return BlockFn<std::decay_t<T>>{std::function<T()>{std::forward<F>(f)}};
}

#include "system/work_manager.hpp"
