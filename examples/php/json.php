<?php
$data = [
    'timestamp' => date('c'),
    'php_version' => PHP_VERSION,
    'memory_limit' => ini_get('memory_limit'),
    'random_number' => rand(1, 100),
    'environment' => [
        'user' => getenv('USER'),
        'home' => getenv('HOME'),
        'path' => getenv('PATH')
    ]
];

echo json_encode($data, JSON_PRETTY_PRINT) . "\n";