#pragma once

#include "data_structures/lazy.tpp"
#include "data_structures/lock.tpp"
#include "fn/continuation.hpp"
#include "system/work_manager.hpp"
#include "time/sleep.hpp"
#include "types/builtin.hpp"
#include "types/utils.hpp"

#include <atomic>
#include <chrono>
#include <cstdint>
#include <functional>
#include <memory>
#include <tuple>
#include <type_traits>
#include <vector>

using namespace std::literals::chrono_literals;

class Fn {
    friend struct WorkManager;

  protected:
    virtual void run() = 0;
    virtual bool done() const = 0;
    virtual void await_all() = 0;

  public:
    virtual ~Fn() = default;
    template <typename... Ts> static auto reference_all(Ts... args);
};

template <typename Ret, typename... Args>
struct ParametricFn : public Fn, Lazy<Ret> {
    using ArgsT = LazyT<std::tuple<std::decay_t<Args>...>>;
    using R = std::decay_t<Ret>;
    using T = std::shared_ptr<ParametricFn<R, Args...>>;

    std::atomic<bool> done_flag{false};
    Locked<std::vector<Continuation>> continuations;
    ArgsT args;
    LazyT<R> ret;

    ParametricFn();
    explicit ParametricFn(LazyT<std::decay_t<Args>>... args) requires(
        sizeof...(Args) > 0);
    explicit ParametricFn(std::add_const_t<std::add_lvalue_reference_t<
                              Args>>... args) requires(sizeof...(Args) > 0);
    virtual ~ParametricFn();

    virtual std::shared_ptr<ParametricFn<Ret, Args...>> clone() const = 0;
    virtual std::tuple<std::shared_ptr<ParametricFn<Ret, Args...>>, LazyT<Ret>>
    clone_with_args(LazyT<std::decay_t<Args>>... args) const;

    virtual LazyT<R>
    body(std::add_lvalue_reference_t<LazyT<std::decay_t<Args>>>...) = 0;
    void run() override;
    void await_all() override;

    template <typename T> static void assign(T &ret, T &return_) {
        //? weird bug means this needs inlining
        if constexpr (is_lazy_v<T>) {
            auto ptr = std::dynamic_pointer_cast<
                LazyPlaceholderBase<remove_lazy_t<T>>>(ret);
            if (ptr == nullptr) {
                ret = return_;
            } else {
                ptr->assign(return_);
            }
        } else if constexpr (is_tuple_v<T>) {
            constexpr auto size = std::tuple_size_v<T>;
            [&]<std::size_t... Is>(std::index_sequence<Is...>) {
                ((assign(std::get<Is>(ret), std::get<Is>(return_))), ...);
            }
            (std::make_index_sequence<size>{});
        }
    }

    virtual void cleanup();
    void cleanup_args();
    bool done() const override;
    R value() const override;
    void add_continuation(Continuation c) override;
};

template <typename F, typename R, typename... A>
struct EasyCloneFn : ParametricFn<R, A...> {
    using ParametricFn<R, A...>::ParametricFn;
    std::shared_ptr<ParametricFn<R, A...>> clone() const override;
};

class FinishWork : public Fn {
    void run() override;
    bool done() const override;
    void await_all() override;
};

template <typename T> struct BlockFn : public ParametricFn<T> {
    std::function<LazyT<T>()> body_fn;
    LazyT<T> body() override;
    explicit BlockFn(std::function<LazyT<T>()> &&f);
    explicit BlockFn(const std::function<LazyT<T>()> &f);
    std::shared_ptr<ParametricFn<T>> clone() const override;
};

template <typename E> struct ClosureRoot {
    LazyT<E> env;
    explicit ClosureRoot(const LazyT<E> &e);
    explicit ClosureRoot();
    virtual ~ClosureRoot();
};

template <typename T, typename E, typename R, typename... A>
struct Closure : ClosureRoot<E>, ParametricFn<R, A...> {
    using ClosureRoot<E>::ClosureRoot;
    std::shared_ptr<ParametricFn<R, A...>> clone() const override;
    virtual ~Closure();
};

template <> struct ClosureRoot<Empty> {};

template <typename T, typename R, typename... A>
struct Closure<T, Empty, R, A...> : ClosureRoot<Empty>, ParametricFn<R, A...> {
    explicit Closure();
    explicit Closure(const Empty &e);
    std::shared_ptr<ParametricFn<R, A...>> clone() const override;
};
