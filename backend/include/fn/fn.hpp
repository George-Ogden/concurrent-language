#pragma once

#include <memory>
#include <type_traits>

class Fn {
  protected:
    const void *_fn = nullptr;
    const std::shared_ptr<void> _env;
    Fn(void *fn, std::shared_ptr<void> env);

  public:
    Fn();
    virtual ~Fn();
};

template <typename R, typename... Args> struct TypedFn : public Fn {
    using T = R (*)(Args..., std::shared_ptr<void>);
    TypedFn(T fn, std::shared_ptr<void> env);
    T fn() const;
    R call(Args... args) const;
};
