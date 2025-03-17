#pragma once

#include "data_structures/lock.hpp"
#include "types/utils.hpp"

#include <array>
#include <atomic>
#include <memory>
#include <optional>
#include <utility>
#include <vector>

struct Work;
struct LazyValue {
    virtual void get_work(std::vector<std::shared_ptr<Work>> &work) = 0;
    virtual ~LazyValue();
};

template <typename T> struct Lazy : LazyValue {
    virtual bool done() = 0;
    virtual T value() = 0;
    virtual T &lvalue() = 0;
    virtual void get_work(std::vector<std::shared_ptr<Work>> &work) override;
    virtual std::shared_ptr<Lazy<T>> as_ref();
};

template <typename T> class LazyConstant : public Lazy<T> {
    T _value;

  public:
    template <typename... Args> LazyConstant(Args &&...);
    bool done() override;
    T value() override;
    T &lvalue() override;
};

template <typename T> class LazyPlaceholder : public Lazy<T> {
    std::atomic<std::shared_ptr<Lazy<T>>> reference = nullptr;
    std::atomic<std::shared_ptr<Work>> work;

  public:
    explicit LazyPlaceholder(std::shared_ptr<Work> work);
    void assign(std::shared_ptr<Lazy<T>> value);
    bool done() override;
    T value() override;
    T &lvalue() override;
    virtual void get_work(std::vector<std::shared_ptr<Work>> &work) override;
    std::shared_ptr<Lazy<T>> as_ref() override;
};

static inline std::shared_ptr<void> null_shared_ptr{nullptr};
static inline std::array<LazyConstant<Bool>, 2> bools = {
    LazyConstant<Bool>{true}, LazyConstant<Bool>{false}};
static inline std::shared_ptr<Lazy<Bool>> lazy_true{null_shared_ptr, &bools[0]};
static inline std::shared_ptr<Lazy<Bool>> lazy_false{null_shared_ptr,
                                                     &bools[1]};

template <typename... Args>
std::shared_ptr<Lazy<Bool>> make_lazy_bool(Args &&...args) {
    if (Bool(args...)) {
        return lazy_true;
    } else {
        return lazy_false;
    }
}

constexpr static inline std::size_t N = 128;
static inline std::array<LazyConstant<Int>, N * 2> integers =
    // cppcheck-suppress syntaxError
    []<std::size_t... Is>(std::index_sequence<Is...>) {
    return std::array<LazyConstant<Int>, N * 2>{LazyConstant<Int>(Is - N)...};
}
(std::make_index_sequence<N * 2>{});
static inline std::array<std::shared_ptr<Lazy<Int>>, N * 2> integer_cache =
    []<std::size_t... Is>(std::index_sequence<Is...>) {
    return std::array<std::shared_ptr<Lazy<Int>>, N * 2>{
        std::shared_ptr<Lazy<Int>>{null_shared_ptr, &integers[Is]}...};
}
(std::make_index_sequence<N * 2>{});

template <typename... Args>
std::shared_ptr<Lazy<Int>> make_lazy_int(Args &&...args) {
    const Int i{args...};
    if (-128 <= i && i < 128) {
        return integer_cache[i + 128];
    }
    return std::make_shared<LazyConstant<Int>>(i);
}

template <typename T, typename... Args>
std::shared_ptr<Lazy<T>> make_lazy(Args &&...args) {
    if constexpr (std::is_same_v<T, Bool>) {
        return make_lazy_bool(std::forward<Args>(args)...);
    } else if constexpr (std::is_same_v<T, Int>) {
        return make_lazy_int(std::forward<Args>(args)...);
    }
    return std::make_shared<LazyConstant<T>>(std::forward<Args>(args)...);
}
