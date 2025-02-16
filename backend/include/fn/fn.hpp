#pragma once

#include <memory>
#include <type_traits>

class Fn {
  protected:
    const void *_fn;
    const std::shared_ptr<void> _env;
    Fn(void *fn, std::shared_ptr<void> env);

  public:
    Fn();
    virtual ~Fn();
};

template <typename R, typename... Args> struct TypedFn : public Fn {
    using T = R (*)(Args...);
    TypedFn(T fn, std::shared_ptr<void> env);
    T fn() const;
};
