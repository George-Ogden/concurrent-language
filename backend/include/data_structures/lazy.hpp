#pragma once

#include "fn/continuation.hpp"
#include "types/utils.hpp"

#include <type_traits>

template <typename T, typename U> struct LazyPlaceholder;

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

    template <typename U>
    requires is_shared_ptr_v<U>
    static LazyT<T> make_placeholders(U fn) {
        if constexpr (is_lazy_v<T>) {
            return std::make_shared<
                LazyPlaceholder<remove_lazy_t<T>, typename U::element_type>>(
                fn);
        } else if constexpr (is_tuple_v<T>) {
            return std::apply(
                [fn](auto &&...args) {
                    return std::make_tuple(
                        Lazy<std::decay_t<decltype(args)>>::make_placeholders(
                            fn)...);
                },
                T{});
        } else {
            return nullptr;
        }
    }
};

template <typename T> class LazyPlaceholderBase : public Lazy<T> {
    LazyT<T> reference = nullptr;
    Locked<std::vector<Continuation>> continuations;

  public:
    void add_continuation(Continuation c) override {
        continuations.acquire();
        if (reference == nullptr) {
            continuations->push_back(c);
            continuations.release();
        } else {
            continuations.release();
            reference->add_continuation(c);
        }
    }
    void assign(LazyT<T> value) {
        continuations.acquire();
        for (Continuation &c : *continuations) {
            value->add_continuation(c);
        }
        continuations->clear();
        reference = value;
        continuations.release();
    }
    bool done() const override {
        return reference != nullptr && reference->done();
    }
    T value() const override { return reference->value(); }
};

template <typename T, typename U>
class LazyPlaceholder : public LazyPlaceholderBase<T> {
    std::shared_ptr<U> reference;

  public:
    explicit LazyPlaceholder(std::shared_ptr<U> reference)
        : reference(reference) {}
};

template <typename T> class LazyConstant : public Lazy<T> {
    const T data;

  public:
    explicit LazyConstant() : data() {}
    explicit LazyConstant(const T &data) : data(data) {}
    explicit LazyConstant(T &&data) : data(std::forward<T>(data)) {}
    template <std::size_t Index>
    explicit LazyConstant(std::integral_constant<std::size_t, Index>)
        : data(std::integral_constant<std::size_t, Index>{}) {}
    template <std::size_t Index, typename U>
    explicit LazyConstant(std::integral_constant<std::size_t, Index>, U &&value)
        : data(std::integral_constant<std::size_t, Index>{},
               std::forward<U>(value)) {}
    bool done() const override { return true; }
    T value() const override { return data; }
    void add_continuation(Continuation c) override {
        Lazy<T>::update_continuation(c);
    }
};
