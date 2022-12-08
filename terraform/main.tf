provider "aws" {
  region = var.region

  default_tags {
    tags = {
      rust-sync = ""
    }
  }
}

resource "aws_dynamodb_table" "user_info_table" {
  name             = "tasks"
  billing_mode     = "PROVISIONED"
  stream_view_type = ""

  tags = {}

  read_capacity  = 2
  write_capacity = 2

  hash_key  = "userId"
  range_key = "SK"

  stream_enabled = false

  point_in_time_recovery {
    enabled = false
  }

  attribute {
    name = "userId"
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

  global_secondary_index {
    name            = "type-data-index"
    hash_key        = "type"
    range_key       = "data"
    write_capacity  = 1
    read_capacity   = 1
    projection_type = "ALL"
  }
}
