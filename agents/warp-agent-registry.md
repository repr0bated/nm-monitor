# Warp AI Agent Registry

This is a comprehensive collection of specialized AI agents for software development tasks. Each agent is an expert in their domain and can be invoked to handle specific development challenges.

## 🔧 Development & Debugging Agents

### **code-reviewer**
Expert code review specialist focusing on quality, security, and maintainability.
- Runs git diff analysis
- Provides structured feedback (Critical/High/Medium/Low)
- Reviews for security vulnerabilities, performance, and best practices
- **Usage**: Invoke after writing or modifying code

### **ai-review** 
Specialized AI/ML code review expert for machine learning projects.
- Model code quality and reproducibility
- Data handling and privacy compliance
- LLM-specific checks (prompt injection, context management)
- Vector database optimization
- **Usage**: Review AI/ML codebases and models

### **smart-debug** / **debug-trace**
Comprehensive debugging environment setup and distributed tracing.
- VS Code debug configurations
- Remote debugging setup
- OpenTelemetry distributed tracing
- Performance profiling and memory leak detection
- **Usage**: Set up debugging infrastructure

### **error-analysis** / **error-trace**
Advanced error tracking and analysis system.
- Error pattern recognition
- Root cause analysis
- Automated error categorization
- Integration with monitoring systems
- **Usage**: Analyze and resolve application errors

## 🏗️ Architecture & Infrastructure Agents

### **api-scaffold**
API development framework generator.
- RESTful API structure generation
- Database integration setup
- Authentication and authorization
- API documentation generation
- **Usage**: Bootstrap new API projects

### **api-mock**
API mocking framework for development and testing.
- Realistic mock service creation
- Scenario-based responses
- Integration with testing frameworks
- **Usage**: Create mock APIs for parallel development

### **data-pipeline**
Data processing pipeline architect.
- ETL/ELT pipeline design
- Stream processing setup
- Data validation and quality checks
- Monitoring and alerting
- **Usage**: Build robust data processing systems

### **db-migrate**
Database migration and schema management expert.
- Safe migration strategies
- Zero-downtime deployments
- Data transformation scripts
- Rollback procedures
- **Usage**: Handle database schema changes

## 🔒 Security & Compliance Agents

### **security-scan** / **security-hardening**
Comprehensive security analysis and hardening.
- Vulnerability scanning
- Security configuration review
- Compliance checking (OWASP, SOC2, GDPR)
- Penetration testing setup
- **Usage**: Secure applications and infrastructure

### **deps-audit** / **deps-upgrade**
Dependency security and upgrade management.
- CVE vulnerability scanning
- License compliance checking
- Automated dependency updates
- Breaking change detection
- **Usage**: Maintain secure and up-to-date dependencies

### **config-validate**
Configuration validation and security.
- Schema-based validation
- Environment-specific rules
- Secret detection and management
- Configuration testing
- **Usage**: Ensure configuration correctness and security

## ☁️ Cloud & DevOps Agents

### **deploy-checklist**
Deployment configuration and procedures.
- Infrastructure as Code (Terraform, K8s)
- CI/CD pipeline setup
- Monitoring and alerting configuration
- Rollback strategies
- **Usage**: Plan and execute safe deployments

### **docker-optimize**
Container optimization and best practices.
- Multi-stage build optimization
- Security hardening
- Size reduction techniques
- Runtime optimization
- **Usage**: Optimize Docker containers

### **cost-optimize**
Cloud infrastructure cost optimization.
- Usage analysis and recommendations
- Reserved instance planning
- Resource rightsizing
- Spot instance strategies
- **Usage**: Reduce cloud infrastructure costs

### **monitor-setup**
Application and infrastructure monitoring.
- Metrics collection setup
- Dashboard creation
- Alert configuration
- SLA monitoring
- **Usage**: Implement comprehensive monitoring

## 📊 Data & Analytics Agents

### **data-validation**
Data quality and validation frameworks.
- Schema validation
- Data profiling and quality metrics
- Anomaly detection
- Data lineage tracking
- **Usage**: Ensure data quality and reliability

### **data-driven-feature**
Feature development with data insights.
- A/B testing framework
- Analytics integration
- Feature flagging
- Metrics-driven development
- **Usage**: Build data-informed features

## 🧪 Testing & Quality Agents

### **accessibility-audit**
WCAG compliance and accessibility testing.
- Automated accessibility scanning
- Manual testing checklists
- Screen reader compatibility
- Keyboard navigation testing
- **Usage**: Ensure application accessibility

### **performance-optimization**
Application performance analysis and optimization.
- Performance profiling
- Bundle analysis
- Runtime optimization
- Load testing
- **Usage**: Improve application performance

### **compliance-check**
Regulatory and industry compliance validation.
- GDPR, HIPAA, SOX compliance
- Audit trail generation
- Policy enforcement
- Documentation generation
- **Usage**: Ensure regulatory compliance

## 🤖 AI & Integration Agents

### **ai-assistant**
AI assistant development framework.
- Natural language processing
- Conversation flow design
- Context management
- LLM integration
- **Usage**: Build intelligent conversational interfaces

### **langchain-agent**
LangChain-based AI agent development.
- Agent orchestration
- Tool integration
- Memory management
- Chain composition
- **Usage**: Create sophisticated AI agents

### **ml-pipeline**
Machine learning pipeline development.
- Model training pipelines
- Feature engineering
- Model versioning and deployment
- MLOps practices
- **Usage**: Build production ML systems

## 📚 Documentation & Maintenance Agents

### **doc-generate**
Automated documentation generation.
- API documentation
- Code comments generation
- README creation
- Architecture diagrams
- **Usage**: Generate comprehensive project documentation

### **code-explain**
Code analysis and explanation generator.
- Code complexity analysis
- Documentation generation
- Refactoring suggestions
- Architecture insights
- **Usage**: Understand and document existing codebases

### **code-migrate**
Code migration and modernization.
- Framework upgrades
- Language migrations
- Legacy code modernization
- Dependency updates
- **Usage**: Migrate codebases to modern technologies

### **refactor-clean**
Code refactoring and cleanup specialist.
- Code smell detection
- Design pattern application
- Performance improvements
- Maintainability enhancements
- **Usage**: Improve code quality and structure

## 🎯 Specialized Agents

### **context-save** / **context-restore**
Project context management for agent coordination.
- Save project state and decisions
- Restore context across sessions
- Maintain agent coordination history
- Enable work continuity
- **Usage**: Manage project context for better agent coordination

### **feature-development** / **full-stack-feature**
End-to-end feature development.
- Full-stack implementation
- Database design
- Frontend/backend integration
- Testing strategy
- **Usage**: Build complete features from requirements

### **git-workflow**
Git workflow optimization and best practices.
- Branching strategies
- Commit message standards
- PR templates and automation
- Release management
- **Usage**: Optimize development workflows

### **onboard**
Team onboarding and project setup.
- Development environment setup
- Documentation creation
- Best practices communication
- Tool configuration
- **Usage**: Onboard new team members effectively

## 📋 Project Management Agents

### **standup-notes**
Development standup and progress tracking.
- Progress summarization
- Blocker identification
- Goal tracking
- Team communication
- **Usage**: Generate standup reports and track progress

### **pr-enhance**
Pull request optimization and review.
- PR description generation
- Review checklist creation
- Automated testing integration
- Merge strategy optimization
- **Usage**: Improve pull request quality and process

### **issue**
Issue tracking and project management.
- Bug report templates
- Feature request processing
- Priority assignment
- Resolution tracking
- **Usage**: Manage project issues effectively

## 🚀 How to Use These Agents

### Direct Invocation
Each agent can be called directly with specific requirements:
```
Use the code-reviewer agent to review the authentication module for security issues.
```

### Chained Operations
Combine agents for complex workflows:
```
1. Use deps-audit to scan for vulnerabilities
2. Use deps-upgrade to plan safe updates  
3. Use deploy-checklist to plan the deployment
```

### Context-Aware Usage
Agents understand project context and can work together:
```
Use context-restore to load the project context, then use the performance-optimization agent to improve the checkout flow.
```

## 🎯 Getting Started

1. **Identify Your Need**: Choose the appropriate agent based on your task
2. **Provide Context**: Include relevant project details and requirements
3. **Specify Scope**: Define what you want the agent to focus on
4. **Review Output**: Carefully review agent recommendations before implementation

## 🔄 Agent Coordination

These agents are designed to work together. Use:
- **context-save** before major work sessions
- **context-restore** when resuming work
- Multiple agents in sequence for complex tasks
- Specialized agents for domain-specific expertise

Each agent provides comprehensive, actionable output with code examples, configurations, and step-by-step instructions tailored to your specific needs.