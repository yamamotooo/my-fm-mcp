//
//  FMMiniPlugIn.cpp
//  FMMiniPlugIn
//
//  Copyright © 2016 - 2024  Claris International Inc.
//  All rights reserved.
//
//  Claris International Inc. grants you a non-exclusive limited license to use this file solely
//  to enable licensees of Claris FileMaker Pro to compile plug-ins for use with Claris products.
//  Redistribution and use in source and binary forms, without modification, are permitted provided
//  that the following conditions are met:
//
//  * Redistributions of source code must retain the above copyright notice, this list of
//  conditions and the following disclaimer.
//
//  * The name Claris International Inc. may not be used to endorse or promote products derived
//  from this software without specific prior written permission.
//
//  THIS SOFTWARE IS PROVIDED BY CLARIS INTERNATIONAL INC. ''AS IS'' AND ANY
//  EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
//  WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//  DISCLAIMED. IN NO EVENT SHALL FILEMAKER, INC. BE LIABLE FOR ANY DIRECT,
//  INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
//  BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
//  DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
//  THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
//  (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
//  THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//

#include "FMWrapper/FMXTypes.h"
#include "FMWrapper/FMXText.h"
#include "FMWrapper/FMXFixPt.h"
#include "FMWrapper/FMXData.h"
#include "FMWrapper/FMXCalcEngine.h"

#ifdef _WIN32
	#include <windows.h>
#else
	#include <sys/socket.h>
	#include <sys/un.h>
	#include <unistd.h>
#endif

#ifdef __APPLE__
	#include "clipboard_mac.h"
#endif

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstring>
#include <memory>
#include <mutex>
#include <queue>
#include <string>
#include <thread>
#include <vector>

// MCP Request Queue ========================================================================
//
// Receives commands from the IPC thread (non-FM main thread) and passes them to the FM
// main thread (kFMXT_Idle), then returns the result back to the IPC thread.

struct McpRequest
{
	std::string command;
	std::string args;     // optional payload (e.g. XML for set_clipboard)
	std::string response;
	bool        done = false;
	std::mutex              mtx;
	std::condition_variable cv;
};

static std::mutex                              gQueueMtx;
static std::queue<std::shared_ptr<McpRequest>> gRequestQueue;

// Common: JSON helpers =====================================================================

static std::string ExtractJsonStr( const std::string& line, const std::string& key )
{
	std::string result;
	auto pos = line.find( "\"" + key + "\"" );
	if ( pos == std::string::npos ) return result;
	auto colon = line.find( ':', pos );
	if ( colon == std::string::npos ) return result;
	auto q1 = line.find( '"', colon + 1 );
	if ( q1 == std::string::npos ) return result;
	auto q2 = q1 + 1;
	while ( q2 < line.size() )
	{
		if ( line[q2] == '\\' )
		{
			++q2;
			if ( q2 < line.size() )
			{
				switch ( line[q2] )
				{
					case '"':  result += '"';  break;
					case '\\': result += '\\'; break;
					case '/':  result += '/';  break;
					case 'n':  result += '\n'; break;
					case 'r':  result += '\r'; break;
					case 't':  result += '\t'; break;
					case 'b':  result += '\b'; break;
					case 'f':  result += '\f'; break;
					default:   result += line[q2]; break;
				}
				++q2;
			}
			continue;
		}
		if ( line[q2] == '"' ) break;
		result += line[q2++];
	}
	return result;
}

// Common: command dispatch =================================================================

static std::string ProcessCommand( const std::string& line )
{
	if ( line.find( "\"ping\"" ) != std::string::npos )
	{
		return "{\"status\":\"ok\",\"message\":\"pong\"}\n";
	}
	else if ( line.find( "\"evaluate\"" ) != std::string::npos )
	{
		std::string exprStr = ExtractJsonStr( line, "expr" );
		auto req = std::make_shared<McpRequest>();
		req->command = "evaluate:" + exprStr;
		{ std::lock_guard<std::mutex> lk( gQueueMtx ); gRequestQueue.push( req ); }
		std::unique_lock<std::mutex> lk( req->mtx );
		if ( req->cv.wait_for( lk, std::chrono::seconds( 5 ), [&req]{ return req->done; } ) )
			return req->response + "\n";
		return "{\"status\":\"error\",\"message\":\"timeout\"}\n";
	}
	else if ( line.find( "\"get_tables\"" ) != std::string::npos )
	{
		auto req = std::make_shared<McpRequest>();
		req->command = "get_tables";
		{ std::lock_guard<std::mutex> lk( gQueueMtx ); gRequestQueue.push( req ); }
		std::unique_lock<std::mutex> lk( req->mtx );
		if ( req->cv.wait_for( lk, std::chrono::seconds( 5 ), [&req]{ return req->done; } ) )
			return req->response + "\n";
		return "{\"status\":\"error\",\"message\":\"timeout\"}\n";
	}
	else if ( line.find( "\"get_fields\"" ) != std::string::npos )
	{
		std::string table = ExtractJsonStr( line, "table" );
		auto req = std::make_shared<McpRequest>();
		req->command = "get_fields:" + table;
		{ std::lock_guard<std::mutex> lk( gQueueMtx ); gRequestQueue.push( req ); }
		std::unique_lock<std::mutex> lk( req->mtx );
		if ( req->cv.wait_for( lk, std::chrono::seconds( 5 ), [&req]{ return req->done; } ) )
			return req->response + "\n";
		return "{\"status\":\"error\",\"message\":\"timeout\"}\n";
	}
	else if ( line.find( "\"set_clipboard\"" ) != std::string::npos )
	{
		std::string xml = ExtractJsonStr( line, "xml" );
		auto req = std::make_shared<McpRequest>();
		req->command = "set_clipboard";
		req->args    = std::move( xml );
		{ std::lock_guard<std::mutex> lk( gQueueMtx ); gRequestQueue.push( req ); }
		std::unique_lock<std::mutex> lk( req->mtx );
		if ( req->cv.wait_for( lk, std::chrono::seconds( 5 ), [&req]{ return req->done; } ) )
			return req->response + "\n";
		return "{\"status\":\"error\",\"message\":\"timeout\"}\n";
	}
	else if ( line.find( "\"get_records\"" ) != std::string::npos )
	{
		std::string table    = ExtractJsonStr( line, "table" );
		std::string limitStr = ExtractJsonStr( line, "limit" );
		int limit = limitStr.empty() ? 50 : std::stoi( limitStr );
		auto req = std::make_shared<McpRequest>();
		req->command = "get_records:" + table + ":" + std::to_string( limit );
		{ std::lock_guard<std::mutex> lk( gQueueMtx ); gRequestQueue.push( req ); }
		std::unique_lock<std::mutex> lk( req->mtx );
		if ( req->cv.wait_for( lk, std::chrono::seconds( 10 ), [&req]{ return req->done; } ) )
			return req->response + "\n";
		return "{\"status\":\"error\",\"message\":\"timeout\"}\n";
	}
	return "{\"status\":\"error\",\"message\":\"unknown command\"}\n";
}

// IPC Server ===============================================================================

static std::thread       gServerThread;
static std::atomic<bool> gRunning { false };

#ifdef _WIN32

// Windows: Named Pipe
// Rust client connects to \\.\pipe\filemaker_mcp

static const wchar_t* kPipeName = L"\\\\.\\pipe\\filemaker_mcp";
static HANDLE         gShutdownEvent = nullptr;

static void IPC_ServerThread()
{
	while ( gRunning )
	{
		// Create a new pipe instance per connection (one connection at a time).
		// Variable named hPipe to avoid conflict with POSIX 'pipe' identifier in MSVC CRT.
		HANDLE hPipe = CreateNamedPipeW(
			kPipeName,
			PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
			PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
			1,          // max instances
			4096, 4096, // I/O buffer sizes
			0, nullptr );
		if ( hPipe == INVALID_HANDLE_VALUE ) break;

		// Wait for client connection; cancellable via gShutdownEvent.
		// Using memset for OVERLAPPED to avoid MSVC aggregate-init issues.
		OVERLAPPED connOv;
		memset( &connOv, 0, sizeof( connOv ) );
		connOv.hEvent = CreateEvent( nullptr, TRUE, FALSE, nullptr );
		if ( !connOv.hEvent ) { CloseHandle( hPipe ); break; }

		ConnectNamedPipe( hPipe, &connOv );

		HANDLE waitH[2] = { connOv.hEvent, gShutdownEvent };
		DWORD  w = WaitForMultipleObjects( 2, waitH, FALSE, INFINITE );
		CloseHandle( connOv.hEvent );

		if ( w != WAIT_OBJECT_0 )
		{
			CloseHandle( hPipe );
			break;
		}

		DWORD tmp;
		GetOverlappedResult( hPipe, &connOv, &tmp, FALSE );

		// Read until newline (max 4096 bytes), reusing one event handle per connection.
		HANDLE hIoEv = CreateEvent( nullptr, TRUE, FALSE, nullptr );
		if ( !hIoEv ) { CloseHandle( hPipe ); continue; }

		std::string line;
		char        ch;
		DWORD       bytesRead;
		bool        ioOk = true;

		while ( ioOk && line.size() < 2097152 )
		{
			OVERLAPPED rov;
			memset( &rov, 0, sizeof( rov ) );
			rov.hEvent = hIoEv;
			ResetEvent( hIoEv );

			if ( !ReadFile( hPipe, &ch, 1, nullptr, &rov ) )
			{
				if ( GetLastError() != ERROR_IO_PENDING ) { ioOk = false; break; }
				WaitForSingleObject( hIoEv, INFINITE );
			}

			if ( !GetOverlappedResult( hPipe, &rov, &bytesRead, FALSE ) || bytesRead != 1 )
				{ ioOk = false; break; }

			if ( ch == '\n' ) break;
			line += ch;
		}

		if ( ioOk && !line.empty() )
		{
			std::string responseStr = ProcessCommand( line );

			OVERLAPPED wov;
			memset( &wov, 0, sizeof( wov ) );
			wov.hEvent = hIoEv;
			ResetEvent( hIoEv );

			DWORD written;
			if ( !WriteFile( hPipe, responseStr.c_str(), static_cast<DWORD>( responseStr.size() ), nullptr, &wov ) )
				if ( GetLastError() == ERROR_IO_PENDING )
					WaitForSingleObject( hIoEv, INFINITE );
			GetOverlappedResult( hPipe, &wov, &written, FALSE );
		}

		CloseHandle( hIoEv );
		FlushFileBuffers( hPipe );
		DisconnectNamedPipe( hPipe );
		CloseHandle( hPipe );
	}
}

static bool IPC_Start()
{
	gShutdownEvent = CreateEvent( nullptr, TRUE, FALSE, nullptr );
	if ( !gShutdownEvent ) return false;
	gRunning = true;
	gServerThread = std::thread( IPC_ServerThread );
	return true;
}

static void IPC_Stop()
{
	gRunning = false;
	if ( gShutdownEvent ) SetEvent( gShutdownEvent );
	if ( gServerThread.joinable() ) gServerThread.join();
	if ( gShutdownEvent ) { CloseHandle( gShutdownEvent ); gShutdownEvent = nullptr; }
}

#else

// Unix: Unix Domain Socket

static const char*      kSocketPath = "/tmp/filemaker_mcp.sock";
static std::atomic<int> gServerFd { -1 };

static void IPC_ServerThread( int serverFd )
{
	while ( gRunning )
	{
		fd_set readfds;
		FD_ZERO( &readfds );
		FD_SET( serverFd, &readfds );
		struct timeval tv { 1, 0 };  // 1-second timeout to check shutdown

		int ret = select( serverFd + 1, &readfds, nullptr, nullptr, &tv );
		if ( ret < 0 ) break;
		if ( ret == 0 ) continue;

		int clientFd = accept( serverFd, nullptr, nullptr );
		if ( clientFd < 0 ) break;

		std::string line;
		char        ch;
		while ( line.size() < 2097152 && read( clientFd, &ch, 1 ) == 1 && ch != '\n' )
			line += ch;

		std::string responseStr = ProcessCommand( line );

		write( clientFd, responseStr.c_str(), responseStr.size() );
		close( clientFd );
	}

	close( serverFd );
	unlink( kSocketPath );
}

static void IPC_Log( const char* msg )
{
	FILE* f = fopen( "/tmp/filemaker_mcp_debug.log", "a" );
	if ( f ) { fprintf( f, "%s\n", msg ); fclose( f ); }
}

static bool IPC_Start()
{
	IPC_Log( "IPC_Start: called" );

	int fd = socket( AF_UNIX, SOCK_STREAM, 0 );
	if ( fd < 0 )
	{
		char buf[128]; snprintf( buf, sizeof(buf), "IPC_Start: socket() failed errno=%d", errno );
		IPC_Log( buf ); return false;
	}

	unlink( kSocketPath );

	struct sockaddr_un addr;
	memset( &addr, 0, sizeof( addr ) );
	addr.sun_family = AF_UNIX;
	strncpy( addr.sun_path, kSocketPath, sizeof( addr.sun_path ) - 1 );

	if ( bind( fd, reinterpret_cast<struct sockaddr*>( &addr ), sizeof( addr ) ) < 0 )
	{
		char buf[128]; snprintf( buf, sizeof(buf), "IPC_Start: bind() failed errno=%d", errno );
		IPC_Log( buf ); close( fd ); return false;
	}
	if ( listen( fd, 5 ) < 0 )
	{
		char buf[128]; snprintf( buf, sizeof(buf), "IPC_Start: listen() failed errno=%d", errno );
		IPC_Log( buf ); close( fd ); return false;
	}

	IPC_Log( "IPC_Start: success, socket created" );
	gServerFd = fd;
	gRunning  = true;
	gServerThread = std::thread( IPC_ServerThread, fd );
	return true;
}

static void IPC_Stop()
{
	gRunning = false;
	int fd = gServerFd.exchange( -1 );
	if ( fd >= 0 ) close( fd );
	if ( gServerThread.joinable() ) gServerThread.join();
}

#endif  // _WIN32

// Plugin ID ===============================================================================

static const char* kMCps( "MCps" );

// Do_PluginInit ===========================================================================

static fmx::ptrtype Do_PluginInit( fmx::int16 version )
{
	char buf[128]; snprintf( buf, sizeof(buf), "Do_PluginInit: version=%d k140=%d kCurrent=%d", (int)version, (int)k140ExtnVersion, (int)kCurrentExtnVersion );
	IPC_Log( buf );

	fmx::ptrtype result( static_cast<fmx::ptrtype>(kDoNotEnable) );

	if (version >= k140ExtnVersion)
	{
		result = kCurrentExtnVersion;
	}

	IPC_Start();

	return result;
}

// Do_PluginShutdown =======================================================================

static void Do_PluginShutdown( fmx::int16 /* version */ )
{
	IPC_Stop();
}

// Do_GetString ============================================================================

static void CopyUTF8StrToUnichar16Str( const char* inStr, fmx::uint32 outStrSize, fmx::unichar16* outStr )
{
	fmx::TextUniquePtr txt;
	txt->Assign( inStr, fmx::Text::kEncoding_UTF8 );
	const fmx::uint32 txtSize( (outStrSize <= txt->GetSize()) ? (outStrSize - 1) : txt->GetSize() );
	txt->GetUnicode( outStr, 0, txtSize );
	outStr[txtSize] = 0;
}

static void Do_GetString( fmx::uint32 whichString, fmx::uint32 /* winLangID */, fmx::uint32 outBufferSize, fmx::unichar16* outBuffer )
{
	switch (whichString)
	{
		case kFMXT_NameStr:
		{
			CopyUTF8StrToUnichar16Str( "FileMakerMCP", outBufferSize, outBuffer );
			break;
		}

		case kFMXT_AppConfigStr:
		{
			CopyUTF8StrToUnichar16Str( "Small example plug-in from FileMaker", outBufferSize, outBuffer );
			break;
		}

		case kFMXT_OptionsStr:
		{
			// Characters 1-4: plug-in ID
			CopyUTF8StrToUnichar16Str( kMCps, outBufferSize, outBuffer );
			// Character 5: always "1"
			outBuffer[4] = '1';
			// Character 6: "Y" to show Configure button, "n" otherwise
			outBuffer[5] = 'n';
			// Character 7: always "n"
			outBuffer[6] = 'n';
			// Character 8: "Y" to receive kFMXT_Init/kFMXT_Shutdown
			outBuffer[7] = 'Y';
			// Character 9: "Y" to receive kFMXT_Idle (required to process FM API calls on main thread)
			outBuffer[8] = 'Y';
			// Character 10: "Y" to receive kFMXT_SessionShutdown and kFMXT_FileShutdown
			outBuffer[9] = 'n';
			// Character 11: always "n"
			outBuffer[10] = 'n';
			// NULL terminator
			outBuffer[11] = 0;
			break;
		}

		case kFMXT_HelpURLStr:
		{
			CopyUTF8StrToUnichar16Str( "http://httpbin.org/get?id=", outBufferSize, outBuffer );
			break;
		}

		default:
		{
			outBuffer[0] = 0;
			break;
		}
	}
}

// Helper: fmx::Text -> std::string (UTF-8) ===============================================

static std::string FMXTextToUTF8( const fmx::Text& txt )
{
	fmx::uint32 charCount = txt.GetSize();
	if ( charCount == 0 ) return "";
	std::vector<char> buf( charCount * 4 + 1, 0 );
	txt.GetBytes( buf.data(), static_cast<fmx::uint32>( buf.size() ),
	              0, static_cast<fmx::uint32>( fmx::Text::kSize_End ), fmx::Text::kEncoding_UTF8 );
	return std::string( buf.data() );
}

// Helper: JSON string escape =============================================================

static std::string JsonEscape( const std::string& s )
{
	std::string r;
	r.reserve( s.size() );
	for ( unsigned char c : s )
	{
		if      ( c == '"'  ) r += "\\\"";
		else if ( c == '\\' ) r += "\\\\";
		else if ( c == '\b' ) r += "\\b";
		else if ( c == '\f' ) r += "\\f";
		else if ( c == '\n' ) r += "\\n";
		else if ( c == '\r' ) r += "\\n";  // FM list separator (CR) treated as newline
		else if ( c == '\t' ) r += "\\t";
		else if ( c < 0x20  ) { /* skip control characters */ }
		else                  r += static_cast<char>( c );
	}
	return r;
}

// Helper: split CR/LF-delimited text into vector =========================================

static std::vector<std::string> SplitLines( const std::string& s )
{
	std::vector<std::string> v;
	std::string token;
	for ( char c : s )
	{
		if ( c == '\r' || c == '\n' )
		{
			if ( !token.empty() ) { v.push_back( token ); token.clear(); }
		}
		else token += c;
	}
	if ( !token.empty() ) v.push_back( token );
	return v;
}

// GetRecordsJSON: fetch records via SQL and return as JSON array ==========================

static std::string GetRecordsJSON( const std::string& tableName, int limit )
{
	fmx::ExprEnvUniquePtr env;
	FMX_SetToCurrentEnv( env.get() );

	auto sqlEscapeSingleQuote = []( const std::string& s ) {
		std::string r; size_t pos = 0, prev = 0;
		while ( (pos = s.find( "'", prev )) != std::string::npos )
			{ r += s.substr( prev, pos - prev ) + "''"; prev = pos + 1; }
		return r + s.substr( prev );
	};

	// FieldClass = 'Normal' かつ Container 以外のフィールド名を取得
	// BaseTableName に tableName を直接渡す（テーブルオカレンス名 = ベース名の場合）
	// FileMaker_Tables 経由の解決は不要（FileMaker_BaseTableFields が直接引ける）
	std::string fieldListCalc =
		"ExecuteSQL ( "
		"\"SELECT FieldName FROM FileMaker_BaseTableFields "
		"WHERE BaseTableName = '" + sqlEscapeSingleQuote( tableName ) + "' "
		"AND FieldClass = 'Normal' "
		"AND FieldType NOT LIKE 'binary%' "
		"ORDER BY FieldId\" "
		"; \"\" ; \"¶\" )";

	fmx::DataUniquePtr fieldListResult;
	fmx::TextUniquePtr fieldListExpr;
	fieldListExpr->Assign( fieldListCalc.c_str(), fmx::Text::kEncoding_UTF8 );

	std::vector<std::string> fieldNames;
	if ( env->Evaluate( *fieldListExpr, *fieldListResult ) == 0 )
	{
		std::string raw = FMXTextToUTF8( fieldListResult->GetAsText() );
		for ( auto& fn : SplitLines( raw ) )
		{
			while ( !fn.empty() && ( fn.back() == ' ' || fn.back() == '\t' ) ) fn.pop_back();
			if ( !fn.empty() ) fieldNames.push_back( fn );
		}
	}

	if ( fieldNames.empty() )
		return "{\"status\":\"error\",\"message\":\"no fields found for table: " + JsonEscape( tableName ) + "\"}";

	// SELECT 列リスト構築: FM calc 文字列内では ""fieldName"" が SQL の "fieldName" になる
	std::string colList;
	for ( const auto& fn : fieldNames )
	{
		if ( !colList.empty() ) colList += ", ";
		colList += "\"\"" + fn + "\"\"";
	}

	std::string fmCalc =
		"ExecuteSQL ( "
		"\"SELECT " + colList + " FROM \"\"" + tableName + "\"\" "
		"FETCH FIRST " + std::to_string( limit ) + " ROWS ONLY\" "
		"; \"\t\" ; \"¶\" )";

	fmx::DataUniquePtr sqlResult;
	fmx::TextUniquePtr sqlExpr;
	sqlExpr->Assign( fmCalc.c_str(), fmx::Text::kEncoding_UTF8 );
	fmx::errcode err = env->Evaluate( *sqlExpr, *sqlResult );

	if ( err != 0 )
		return "{\"status\":\"error\",\"message\":\"SQL failed\",\"code\":" + std::to_string( err ) + "}";

	std::string rawData = FMXTextToUTF8( sqlResult->GetAsText() );

	// fields 配列（ヘッダ）を JSON に含める
	std::string json = "{\"status\":\"ok\",\"table\":\"" + JsonEscape( tableName ) + "\",\"fields\":[";
	for ( size_t i = 0; i < fieldNames.size(); ++i )
	{
		if ( i ) json += ',';
		json += '"'; json += JsonEscape( fieldNames[i] ); json += '"';
	}
	json += "],\"records\":[";

	bool firstRow = true;
	for ( const auto& rowStr : SplitLines( rawData ) )
	{
		std::vector<std::string> cols;
		std::string col;
		for ( char c : rowStr )
		{
			if ( c == '\t' ) { cols.push_back( col ); col.clear(); }
			else              col += c;
		}
		cols.push_back( col );

		if ( !firstRow ) json += ',';
		firstRow = false;
		json += '[';
		for ( size_t i = 0; i < cols.size(); ++i )
		{
			if ( i ) json += ',';
			json += '"'; json += JsonEscape( cols[i] ); json += '"';
		}
		json += ']';
	}
	json += "]}";
	return json;
}

// GetFieldsJSON: return field names, types, and repetitions for a given table
// tableName: テーブルオカレンス名（空の場合は現在のレイアウトのテーブルを使用）
//            Table occurrence name (if empty, uses current layout table)

static std::string GetFieldsJSON( const std::string& tableName = "" )
{
	fmx::ExprEnvUniquePtr env;
	FMX_SetToCurrentEnv( env.get() );

	// Get current layout table name (テーブルオカレンス)
	fmx::DataUniquePtr layoutTableResult;
	fmx::TextUniquePtr layoutTableExpr;
	layoutTableExpr->Assign( "Get ( LayoutTableName )", fmx::Text::kEncoding_UTF8 );
	std::string tableOccurrence;
	if ( env->Evaluate( *layoutTableExpr, *layoutTableResult ) == 0 )
	{
		tableOccurrence = FMXTextToUTF8( layoutTableResult->GetAsText() );
		tableOccurrence.erase( std::remove( tableOccurrence.begin(), tableOccurrence.end(), '\r' ), tableOccurrence.end() );
		tableOccurrence.erase( std::remove( tableOccurrence.begin(), tableOccurrence.end(), '\n' ), tableOccurrence.end() );
	}

	// If tableName is provided, use it; otherwise use current layout table
	if ( !tableName.empty() )
		tableOccurrence = tableName;

	if ( tableOccurrence.empty() )
		return "{\"status\":\"error\",\"message\":\"No table name provided and Get(LayoutTableName) failed\"}";

	// ExecuteSQL関数をEvaluate経由で実行（サブクエリを1回で実行）
	// シングルクォートをエスケープ
	std::string escapedTableName = tableOccurrence;
	size_t pos = 0;
	while ( (pos = escapedTableName.find( "'", pos )) != std::string::npos )
	{
		escapedTableName.replace( pos, 1, "''" );
		pos += 2;
	}
	
	// FileMakerのExecuteSQL関数を使ってサブクエリを実行
	// 列を明示的に指定: TableOccurrenceName, FieldName, FieldType, FieldClass, FieldReps, FieldId
	std::string fmCalc = 
		"ExecuteSQL ( "
		"\"SELECT "
		"'" + tableOccurrence + "' AS TableOccurrenceName, "
		"FieldName, "
		"FieldType, "
		"FieldClass, "
		"FieldReps, "
		"FieldId "
		"FROM FileMaker_BaseTableFields "
		"WHERE FileMaker_BaseTableFields.BaseTableName = ("
		"  SELECT FileMaker_Tables.BaseTableName "
		"  FROM FileMaker_Tables "
		"  WHERE FileMaker_Tables.TableName = '" + escapedTableName + "'"
		")\" ; "
		"\"	\" ; "  // 列区切り（タブ）
		"\"¶\" )";  // 行区切り（改行）
	
	fmx::DataUniquePtr sqlResult;
	fmx::TextUniquePtr sqlExpr;
	sqlExpr->Assign( fmCalc.c_str(), fmx::Text::kEncoding_UTF8 );
	fmx::errcode err = env->Evaluate( *sqlExpr, *sqlResult );
	
	if ( err != 0 )
	{
		// Rustのデバッグメッセージ形式に合わせたエラーレスポンス
		std::string errMsg = "{\"status\":\"error\","
		                     "\"message\":\"SQL query failed\","
		                     "\"code\":" + std::to_string( err ) + ","
		                     "\"table\":\"" + JsonEscape( tableOccurrence ) + "\","
		                     "\"baseTable\":\"\","
		                     "\"fileName\":\"\","
		                     "\"sql\":\"" + JsonEscape( fmCalc ) + "\"}";
		return errMsg;
	}

	// 結果を解析
	std::string rawData = FMXTextToUTF8( sqlResult->GetAsText() );
	std::vector<std::string> rows = SplitLines( rawData );
	
	if ( rows.empty() )
		return "{\"status\":\"error\",\"message\":\"no fields found for table: " + JsonEscape( tableOccurrence ) + "\"}";

	std::string json = "{\"status\":\"ok\",\"table\":\"" + JsonEscape( tableOccurrence ) + "\",\"fields\":[";
	bool first = true;

	for ( const auto& rowStr : rows )
	{
		// タブ区切りで分割（6列: TableOccurrenceName, FieldName, FieldType, FieldClass, FieldReps, FieldId）
		std::vector<std::string> cols;
		std::string col;
		for ( char c : rowStr )
		{
			if ( c == '\t' ) { cols.push_back( col ); col.clear(); }
			else             col += c;
		}
		cols.push_back( col );

		if ( cols.empty() || cols[0].empty() ) continue;

		std::string tableOccurrenceName = cols[0];
		std::string fieldName = (cols.size() > 1) ? cols[1] : "";
		std::string fieldType = (cols.size() > 2) ? cols[2] : "varchar";
		std::string fieldClass = (cols.size() > 3) ? cols[3] : "Normal";
		int repetitions = (cols.size() > 4) ? std::atoi( cols[4].c_str() ) : 1;
		std::string fieldId = (cols.size() > 5) ? cols[5] : "0";

		// FieldType を FileMaker の型名に変換
		// "varchar" → "Text", "decimal" → "Number", "timestamp" → "Timestamp" など
		std::string displayType = fieldType;
		if ( fieldType.find( "varchar" ) == 0 ) displayType = "Text";
		else if ( fieldType.find( "decimal" ) == 0 ) displayType = "Number";
		else if ( fieldType == "timestamp" ) displayType = "Timestamp";
		else if ( fieldType == "date" ) displayType = "Date";
		else if ( fieldType == "time" ) displayType = "Time";
		else if ( fieldType.find( "binary" ) == 0 ) displayType = "Container";

		// FieldClass が "Calculated" または "Summary" の場合は型を上書き
		if ( fieldClass == "Calculated" || fieldClass == "Calculation" )
			displayType = "Calculation";
		else if ( fieldClass == "Summary" )
			displayType = "Summary";

		if ( !first ) json += ',';
		first = false;
		json += "{\"tableOccurrence\":\"" + JsonEscape( tableOccurrenceName ) + "\","
		      + "\"name\":\"" + JsonEscape( fieldName ) + "\","
		      + "\"id\":" + fieldId + ","
		      + "\"type\":\"" + JsonEscape( displayType )  + "\","
		      + "\"repetitions\":" + std::to_string( repetitions ) + "}";
	}

	json += "]}";
	return json;
}

// GetTablesJSON: evaluate TableNames() and return as JSON array ===========================

static std::string GetTablesJSON()
{
	fmx::ExprEnvUniquePtr env;
	fmx::DataUniquePtr    result;
	fmx::TextUniquePtr    expr;

	// FMX_SetToCurrentEnv injects the current FM session/file context.
	// Without this, Get(FileName) / DatabaseNames returns empty.
	FMX_SetToCurrentEnv( env.get() );

	expr->Assign( "TableNames ( GetValue ( DatabaseNames ; 1 ) )", fmx::Text::kEncoding_UTF8 );

	fmx::errcode err = env->Evaluate( *expr, *result );
	if ( err != 0 )
	{
		return "{\"status\":\"error\",\"message\":\"evaluate failed, code=" + std::to_string( err ) + "\"}";
	}

	// FM list functions return CR (ASCII 13) delimited values
	std::string raw = FMXTextToUTF8( result->GetAsText() );

	std::string json = "{\"status\":\"ok\",\"tables\":[";
	bool        first = true;
	std::string token;

	for ( char c : raw )
	{
		if ( c == '\r' || c == '\n' )
		{
			if ( !token.empty() )
			{
				if ( !first ) json += ',';
				json += '"';
				json += JsonEscape( token );
				json += '"';
				first = false;
				token.clear();
			}
		}
		else
		{
			token += c;
		}
	}
	if ( !token.empty() )
	{
		if ( !first ) json += ',';
		json += '"';
		json += JsonEscape( token );
		json += '"';
	}

	json += "]}";
	return json;
}

// Do_PluginIdle ===========================================================================

static void Do_PluginIdle( FMX_IdleLevel idleLevel, fmx::ptrtype /* sessionId */ )
{
	std::shared_ptr<McpRequest> req;
	{
		std::lock_guard<std::mutex> lk( gQueueMtx );
		if ( gRequestQueue.empty() ) return;
		// set_clipboard doesn't call FM API, so it's safe even during kFMXT_Unsafe.
		// All other commands require FM API and must wait for a safe idle level.
		if ( idleLevel == kFMXT_Unsafe && gRequestQueue.front()->command != "set_clipboard" ) return;
		req = gRequestQueue.front();
		gRequestQueue.pop();
	}

	std::string responseJson;
	if ( req->command == "get_tables" )
	{
		responseJson = GetTablesJSON();
	}
	else if ( req->command == "set_clipboard" )
	{
#ifdef __APPLE__
		int ok = SetFileMakerClipboard( req->args.c_str(), static_cast<int>( req->args.size() ) );
		responseJson = ok
		    ? "{\"status\":\"ok\",\"message\":\"クリップボードにコピーしました\"}"
		    : "{\"status\":\"error\",\"message\":\"クリップボードへの書き込みに失敗しました\"}";
#else
		responseJson = "{\"status\":\"error\",\"message\":\"set_clipboard is not supported on this platform\"}";
#endif
	}
	else if ( req->command.rfind( "get_fields:", 0 ) == 0 )
	{
		std::string table = req->command.substr( 11 );
		responseJson = GetFieldsJSON( table );
	}
	else if ( req->command.rfind( "get_records:", 0 ) == 0 )
	{
		// "get_records:<table>:<limit>"
		std::string rest = req->command.substr( 12 );
		auto sep = rest.rfind( ':' );
		std::string table = rest.substr( 0, sep );
		int limit = (sep != std::string::npos) ? std::stoi( rest.substr( sep + 1 ) ) : 50;
		responseJson = GetRecordsJSON( table, limit );
	}
	else if ( req->command.rfind( "evaluate:", 0 ) == 0 )
	{
		// "evaluate:<expr>" -- evaluate arbitrary FileMaker expression
		std::string exprStr = req->command.substr( 9 );

		fmx::ExprEnvUniquePtr env;
		fmx::DataUniquePtr    result;
		fmx::TextUniquePtr    expr;
		FMX_SetToCurrentEnv( env.get() );
		expr->Assign( exprStr.c_str(), fmx::Text::kEncoding_UTF8 );

		fmx::errcode err = env->Evaluate( *expr, *result );
		if ( err != 0 )
		{
			responseJson = "{\"status\":\"error\",\"code\":" + std::to_string( err )
			             + ",\"message\":\"evaluate failed\"}";
		}
		else
		{
			std::string raw = FMXTextToUTF8( result->GetAsText() );
			responseJson = "{\"status\":\"ok\",\"result\":\"" + JsonEscape( raw ) + "\"}";
		}
	}
	else
	{
		responseJson = "{\"status\":\"error\",\"message\":\"unknown command\"}";
	}

	{
		std::lock_guard<std::mutex> lk( req->mtx );
		req->response = std::move( responseJson );
		req->done     = true;
	}
	req->cv.notify_one();
}

// Do_PluginPrefs ==========================================================================

static void Do_PluginPrefs( void )
{
}

// Do_SessionNotifications =================================================================

static void Do_SessionNotifications( fmx::uint64 /* sessionID */ )
{
}

// Do_FileNotifications ====================================================================

static void Do_FilenNotifications( fmx::uint64 /* sessionID */, fmx::uint64 /* fileID */ )
{
}

// Do_SchemaNotifications ====================================================================

static void Do_SchemaNotifications( char* /* json utf8 text */, fmx::uint64 /* json length */ )
{
}

// FMExternCallProc ========================================================================

FMX_ExternCallPtr gFMX_ExternCallPtr( nullptr );

void FMX_ENTRYPT FMExternCallProc( FMX_ExternCallPtr pb )
{
	gFMX_ExternCallPtr = pb;

	switch (pb->whichCall)
	{
		case kFMXT_Init:
			pb->result = Do_PluginInit( pb->extnVersion );
			break;

		case kFMXT_Idle:
			Do_PluginIdle( pb->parm1, pb->parm2 );
			break;

		case kFMXT_Shutdown:
			Do_PluginShutdown( pb->extnVersion );
			break;

		case kFMXT_DoAppPreferences:
			Do_PluginPrefs();
			break;

		case kFMXT_GetString:
			Do_GetString( static_cast<fmx::uint32>(pb->parm1), static_cast<fmx::uint32>(pb->parm2), static_cast<fmx::uint32>(pb->parm3), reinterpret_cast<fmx::unichar16*>(pb->result) );
			break;

		case kFMXT_SessionShutdown:
			Do_SessionNotifications( pb->parm2 );
			break;

		case kFMXT_FileShutdown:
			Do_FilenNotifications( pb->parm2, pb->parm3 );
			break;

		case kFMXT_SchemaChange:
			Do_SchemaNotifications( reinterpret_cast<char*>(pb->parm2), pb->parm3 );
			break;

	} // switch whichCall

} // FMExternCallProc
