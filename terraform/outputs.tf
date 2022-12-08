# Output value definitions

output "environment_table_name" {
  description = "Name of the DynamoDB table"
  value       = aws_dynamodb_table.user_info_table.name
}

output "environment_table_arn" {
  description = "ARN of the DynamoDB table"
  value       = aws_dynamodb_table.user_info_table.arn
}
