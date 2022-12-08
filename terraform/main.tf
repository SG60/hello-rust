provider "aws" {
  region = var.region

  default_tags {
    tags = {
      rust-sync = ""
    }
  }
}

resource "aws_dynamodb_table" "user_info_table" {
  name = "tasks"
  billing_mode = "PROVISIONED"

  read_capacity  = 2
  write_capacity = 2
  hash_key       = "UserId"
  range_key      = "SK"

  attribute {
    name = "UserId"
    type = "S"
  }

  attribute {
    name = "SK"
    type = "S"
  }

  attribute {
    name = "type"
    type = "S"
  }

  attribute {
    name = "data"
    type = "S"
  }

  ttl {
    attribute_name = "TimeToExist"
    enabled        = false
  }

  global_secondary_index {
    name               = "type-data-index"
    hash_key           = "type"
    range_key          = "data"
    write_capacity     = 1
    read_capacity      = 1
    projection_type    = "ALL"
  }
}
