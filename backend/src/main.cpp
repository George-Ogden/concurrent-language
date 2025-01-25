#include "main/main.hpp"
#include "system/work_manager.hpp"

#include <iostream>

int main() {
    std::shared_ptr<Main> main = std::make_shared<Main>();
    WorkManager::run(main);
    std::cout << main->value() << std::endl;
    return 0;
}
