#pragma once

#include "fn/continuation.hpp"

#include <type_traits>

template <typename T> struct Lazy {
    virtual bool done() const = 0;
    virtual T value() const = 0;
    virtual void add_continuation(Continuation c) = 0;
    virtual ~Lazy() = default;
    void update_continuation(Continuation c) {
        if (c.remaining->fetch_sub(1, std::memory_order_relaxed) == 1) {
            delete c.remaining;
            c.valid->acquire();
            if (**c.valid) {
                **c.valid = false;
                c.counter.fetch_add(1, std::memory_order_relaxed);
                c.valid->release();
            } else {
                c.valid->release();
                delete c.valid;
            }
        }
    }
    template <typename U> static auto extract_value(const U &v) { return v; }
    template <typename... Us>
    static auto extract_value(const std::tuple<Us...> &v) {
        return std::apply(
            [](const auto &...args) {
                return std::make_tuple(extract_value(args)...);
            },
            v);
    }
    template <typename U>
    static auto extract_value(const std::shared_ptr<Lazy<U>> &v) {
        return v->value();
    }
};

template <typename T> class LazyConstant : public Lazy<T> {
    const T data;

  public:
    explicit LazyConstant(const T &data) : data(data) {}
    bool done() const override { return true; }
    T value() const override { return data; }
    void add_continuation(Continuation c) override {
        Lazy<T>::update_continuation(c);
    }
};

template <typename> struct is_lazy : std::false_type {};

template <typename T>
struct is_lazy<std::shared_ptr<Lazy<T>>> : std::true_type {};

template <typename T> inline constexpr bool is_lazy_v = is_lazy<T>::value;

template <typename T> struct lazy_type {
    using type = std::shared_ptr<Lazy<T>>;
};

template <typename T> using LazyT = typename lazy_type<T>::type;

template <typename T> struct lazy_type<std::shared_ptr<Lazy<T>>> {
    using type = std::shared_ptr<Lazy<T>>;
};

template <typename... Ts> struct lazy_type<std::tuple<Ts...>> {
    using type = std::tuple<LazyT<Ts>...>;
};
