<div align="center">

# hello-rust

frontend - https://github.com/SG60/hello-rust-frontend
https://www.notion.so/samgreening/Notion-Block-Sync-b0735065a45d48d6aeaef63bf07b7a96

<details>
<summary>
Deployed to AWS?!
</summary>

https://eu-west-2.console.aws.amazon.com/dynamodbv2/home?region=eu-west-2#item-explorer?initialTagKey=&table=tasks

</details>

[![Rust](https://github.com/SG60/hello-rust/actions/workflows/rust.yml/badge.svg)](https://github.com/SG60/hello-rust/actions/workflows/rust.yml)

</div>

---

Uses Just (see the justfile) for running scripts etc.


https://eu-west-2.console.aws.amazon.com/dynamodbv2/home?region=eu-west-2#item-explorer?initialTagKey=&table=tasks

# DB Design

https://docs.rs/aws-sdk-dynamodb/latest/aws_sdk_dynamodb/
https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/GettingStarted.html
https://github.com/awslabs/aws-sdk-rust/tree/main/examples/dynamodb
https://docs.aws.amazon.com/sdk-for-rust/latest/dg/getting-started.html
https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/best-practices.html

The DB just stores sync settings and refresh tokens.

## DB Schema

<table>
<tr>
	<th scope="col" colspan=2>PK</th>
	<th scope="col" colspan=99999>Attributes</th>
</tr>
<tr>
	<th scope="col">userId</th>
	<th scope="col">SK</th>
	<th scope="col">type (GSI-1-PK)</th>
	<th scope="col">data (GSI-1-SK)</th>
	<th scope="col" colspan=99999></th>
</tr>
<tbody>
<tr><td rowSpan=0>firebase auth user id
	<tr><td rowspan=2>userDetails
		<th>type	<th>data	<th>notion bot_id	<th>googleRefreshToken	<th>notionAccessToken	<th>other stuff
		<tr><td>userDetails<td>ACTIVE	<td>notionB#bot_id	<td>asdfasefa		<td>asdfasefa		<td>workspace name, workspace emoji, etc.
	<tr><td rowspan=2>sync#0
		<th>type	<th>data (next sync timestamp) (or null)<th>last sync<th>notionDatabase<th>googleCalendar<th>notionTitleId<th>notionDoneId
		<tr><td>sync	<td>SCHEDULED#2007-04-05T14:30Z	<td>LAST#2007-04-05T14:30Z<td>asdfase<td>asdf3<td>flkjhs<td>asdfasefa
</tbody>
</table>

GSI-1 will be:
<table><td>type (e.g. userDetails)<td>data</table>

This should allow sorting all active syncs (e.g. 'sync' + startsWith 'SCHEDULED#') and other useful queries.

Probably doesn't need rows for items. Maybe they can all just be stored in memory.

https://eu-west-2.console.aws.amazon.com/dynamodbv2/home?region=eu-west-2#item-explorer?initialTagKey=&table=tasks

# Notion Integration

https://www.notion.so/my-integrations/public/f8014299c7f64cac8315d858c2aab2c8

# Postman Workspace

https://web.postman.co/workspace/fe759fe4-0286-4679-860f-6dc84d8af0fc
