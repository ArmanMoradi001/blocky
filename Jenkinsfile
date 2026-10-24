pipeline {
    agent any

    options {
        timestamps()
        timeout(time: 20, unit: 'MINUTES')
    }

    stages {
        stage('Checkout') {
            steps {
                checkout scm
            }
        }

        stage('Check formatting') {
            steps {
                sh 'cargo fmt --check'
            }
        }

        stage('Test') {
            steps {
                sh 'cargo test --locked'
            }
        }

        stage('Build') {
            steps {
                sh 'cargo build --release --locked'
            }
        }
    }
}