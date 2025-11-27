# ============================================================================
# LLM Inference Gateway - Terraform Variables
# ============================================================================

variable "aws_region" {
  description = "AWS region for resources"
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Environment name (dev, staging, production)"
  type        = string
  validation {
    condition     = contains(["dev", "staging", "production"], var.environment)
    error_message = "Environment must be dev, staging, or production."
  }
}

variable "project_name" {
  description = "Project name for resource naming"
  type        = string
  default     = "llm-gateway"
}

variable "domain_name" {
  description = "Domain name for the API"
  type        = string
}

# ============================================================================
# VPC Configuration
# ============================================================================

variable "vpc_cidr" {
  description = "CIDR block for VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "availability_zones" {
  description = "Availability zones for resources"
  type        = list(string)
  default     = ["us-east-1a", "us-east-1b", "us-east-1c"]
}

variable "private_subnet_cidrs" {
  description = "CIDR blocks for private subnets"
  type        = list(string)
  default     = ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"]
}

variable "public_subnet_cidrs" {
  description = "CIDR blocks for public subnets"
  type        = list(string)
  default     = ["10.0.101.0/24", "10.0.102.0/24", "10.0.103.0/24"]
}

# ============================================================================
# LLM Provider Secrets
# ============================================================================

variable "openai_api_key" {
  description = "OpenAI API Key"
  type        = string
  sensitive   = true
}

variable "anthropic_api_key" {
  description = "Anthropic API Key"
  type        = string
  sensitive   = true
}

variable "azure_api_key" {
  description = "Azure OpenAI API Key"
  type        = string
  sensitive   = true
  default     = ""
}

variable "google_api_key" {
  description = "Google Gemini API Key"
  type        = string
  sensitive   = true
  default     = ""
}

# ============================================================================
# Tags
# ============================================================================

variable "tags" {
  description = "Common tags for all resources"
  type        = map(string)
  default     = {}
}
