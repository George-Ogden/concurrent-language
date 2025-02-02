#include "main/main.hpp"
#include "system/work_manager.hpp"
#include "time/utils.hpp"

#include <iostream>

int main() {
    auto start = time_utils::now();

    std::shared_ptr<Main> main = std::make_shared<Main>();
    WorkManager::run(main);

    auto end = time_utils::now();

    std::cout << main->value() << std::endl;

    auto duration = time_utils::time_delta(start, end);
    std::cerr << "Execution time: " << duration << " ns" << std::endl;

    return 0;
}
