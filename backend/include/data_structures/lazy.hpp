#pragma once

#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
#include "types/utils.hpp"

#include <memory>
#include <tuple>
#include <type_traits>
#include <vector>

template <typename T, typename U> class LazyPlaceholder;

template <typename T> struct Lazy {
    virtual bool done() const = 0;
    virtual T value() const = 0;
    virtual void add_continuation(Continuation c) = 0;
    virtual ~Lazy() = default;
    void update_continuation(Continuation c);

    template <typename U> static auto extract_value(const U &v);
    template <typename... Us>
    static auto extract_value(const std::tuple<Us...> &v);
    template <typename U>
    static auto extract_value(const std::shared_ptr<Lazy<U>> &v);

    template <typename U>
    requires is_shared_ptr_v<U>
    static LazyT<T> make_placeholders(U fn);
};

template <typename T> class LazyPlaceholderBase : public Lazy<T> {
    LazyT<T> reference = nullptr;
    Locked<std::vector<Continuation>> continuations;

  public:
    void add_continuation(Continuation c) override;
    void assign(LazyT<T> value);
    bool done() const override;
    T value() const override;
};

template <typename T, typename U>
class LazyPlaceholder : public LazyPlaceholderBase<T> {
    std::shared_ptr<U> reference;

  public:
    explicit LazyPlaceholder(std::shared_ptr<U> reference);
};

template <typename T> class LazyConstant : public Lazy<T> {
    const T data;

  public:
    explicit LazyConstant();
    explicit LazyConstant(const T &data);
    explicit LazyConstant(T &&data);
    template <std::size_t Index>
    explicit LazyConstant(std::integral_constant<std::size_t, Index>);
    template <std::size_t Index, typename U>
    explicit LazyConstant(std::integral_constant<std::size_t, Index>,
                          U &&value);
    bool done() const override;
    T value() const override;
    void add_continuation(Continuation c) override;
};
