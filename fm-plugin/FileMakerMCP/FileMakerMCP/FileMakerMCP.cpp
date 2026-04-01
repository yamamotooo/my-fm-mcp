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

static bool IPC_Start()
{
	int fd = socket( AF_UNIX, SOCK_STREAM, 0 );
	if ( fd < 0 ) return false;

	unlink( kSocketPath );

	struct sockaddr_un addr;
	memset( &addr, 0, sizeof( addr ) );
	addr.sun_family = AF_UNIX;
	strncpy( addr.sun_path, kSocketPath, sizeof( addr.sun_path ) - 1 );

	if ( bind( fd, reinterpret_cast<struct sockaddr*>( &addr ), sizeof( addr ) ) < 0 )
	{
		close( fd );
		return false;
	}
	if ( listen( fd, 5 ) < 0 )
	{
		close( fd );
		return false;
	}

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

	fmx::DataUniquePtr fnResult;
	fmx::TextUniquePtr fnExpr;
	fnExpr->Assign( "GetValue ( DatabaseNames ; 1 )", fmx::Text::kEncoding_UTF8 );
	if ( env->Evaluate( *fnExpr, *fnResult ) != 0 )
		return "{\"status\":\"error\",\"message\":\"DatabaseNames failed\"}";
	std::string fileName = FMXTextToUTF8( fnResult->GetAsText() );
	fileName.erase( std::remove( fileName.begin(), fileName.end(), '\r' ), fileName.end() );
	fileName.erase( std::remove( fileName.begin(), fileName.end(), '\n' ), fileName.end() );

	fmx::DataUniquePtr fieldsResult;
	fmx::TextUniquePtr fieldsExpr;
	std::string fieldsExprStr = "FieldNames ( \"" + fileName + "\" ; \"" + tableName + "\" )";
	fieldsExpr->Assign( fieldsExprStr.c_str(), fmx::Text::kEncoding_UTF8 );
	if ( env->Evaluate( *fieldsExpr, *fieldsResult ) != 0 )
		return "{\"status\":\"error\",\"message\":\"FieldNames failed\"}";
	std::vector<std::string> fields = SplitLines( FMXTextToUTF8( fieldsResult->GetAsText() ) );
	if ( fields.empty() )
		return "{\"status\":\"error\",\"message\":\"no fields found\"}";

	std::string sql = "SELECT * FROM \"" + tableName + "\" FETCH FIRST "
	                + std::to_string( limit ) + " ROWS ONLY";
	fmx::TextUniquePtr sqlText, fileText;
	sqlText->Assign( sql.c_str(), fmx::Text::kEncoding_UTF8 );
	fileText->Assign( fileName.c_str(), fmx::Text::kEncoding_UTF8 );
	fmx::DataVectUniquePtr params;
	fmx::DataUniquePtr sqlResult;
	fmx::errcode err = env->ExecuteFileSQLTextResult( *sqlText, *fileText, *params,
	                                                   *sqlResult,
	                                                   '\t',
	                                                   '\r' );
	if ( err != 0 )
		return "{\"status\":\"error\",\"message\":\"SQL failed\",\"code\":" + std::to_string( err ) + "}";

	std::string rawData = FMXTextToUTF8( sqlResult->GetAsText() );
	std::string json = "{\"status\":\"ok\",\"table\":\"" + JsonEscape( tableName ) + "\",\"records\":[";

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
			json += '"';
			json += JsonEscape( cols[i] );
			json += '"';
		}
		json += ']';
	}
	json += "]}";
	return json;
}

// GetFieldsJSON: return field names, types, and repetitions for a given table ==============

static std::string GetFieldsJSON( const std::string& tableName )
{
	fmx::ExprEnvUniquePtr env;
	FMX_SetToCurrentEnv( env.get() );

	// Get the current database filename
	fmx::DataUniquePtr fnResult;
	fmx::TextUniquePtr fnExpr;
	fnExpr->Assign( "GetValue ( DatabaseNames ; 1 )", fmx::Text::kEncoding_UTF8 );
	if ( env->Evaluate( *fnExpr, *fnResult ) != 0 )
		return "{\"status\":\"error\",\"message\":\"DatabaseNames failed\"}";
	std::string fileName = FMXTextToUTF8( fnResult->GetAsText() );
	fileName.erase( std::remove( fileName.begin(), fileName.end(), '\r' ), fileName.end() );
	fileName.erase( std::remove( fileName.begin(), fileName.end(), '\n' ), fileName.end() );

	// Get field names for the table
	fmx::DataUniquePtr namesResult;
	fmx::TextUniquePtr namesExpr;
	std::string namesExprStr = "FieldNames ( \"" + fileName + "\" ; \"" + tableName + "\" )";
	namesExpr->Assign( namesExprStr.c_str(), fmx::Text::kEncoding_UTF8 );
	if ( env->Evaluate( *namesExpr, *namesResult ) != 0 )
		return "{\"status\":\"error\",\"message\":\"FieldNames failed\"}";

	std::vector<std::string> fieldNames = SplitLines( FMXTextToUTF8( namesResult->GetAsText() ) );
	if ( fieldNames.empty() )
		return "{\"status\":\"error\",\"message\":\"no fields found\"}";

	// Get field IDs (same order as FieldNames)
	fmx::DataUniquePtr idsResult;
	fmx::TextUniquePtr idsExpr;
	std::string idsExprStr = "FieldIDs ( \"" + fileName + "\" ; \"" + tableName + "\" )";
	idsExpr->Assign( idsExprStr.c_str(), fmx::Text::kEncoding_UTF8 );
	std::vector<std::string> fieldIds;
	if ( env->Evaluate( *idsExpr, *idsResult ) == 0 )
		fieldIds = SplitLines( FMXTextToUTF8( idsResult->GetAsText() ) );

	std::string json = "{\"status\":\"ok\",\"fields\":[";
	bool first = true;

	for ( size_t fi = 0; fi < fieldNames.size(); ++fi )
	{
		const auto& fieldName = fieldNames[fi];
		std::string fieldId = (fi < fieldIds.size()) ? fieldIds[fi] : "0";
		// FieldType() returns "FieldClass, DataType" e.g. "Normal, Text" / "Calculated, Number"
		fmx::DataUniquePtr typeResult;
		fmx::TextUniquePtr typeExpr;
		std::string typeExprStr = "FieldType ( \"" + fileName + "\" ; \"" + fieldName + "\" )";
		typeExpr->Assign( typeExprStr.c_str(), fmx::Text::kEncoding_UTF8 );

		std::string fieldType = "Text";
		if ( env->Evaluate( *typeExpr, *typeResult ) == 0 )
		{
			std::string raw = FMXTextToUTF8( typeResult->GetAsText() );
			while ( !raw.empty() && ( raw.back() == '\r' || raw.back() == '\n' ) )
				raw.pop_back();

			// FieldType() returns space-separated tokens: "Standard Text Unindexed 1"
			// Token[0]: Standard | Calculated | Summary | Global
			// Token[1]: Text | Number | Date | Time | Timestamp | Container
			auto sp1 = raw.find( ' ' );
			if ( sp1 != std::string::npos )
			{
				std::string fieldClass = raw.substr( 0, sp1 );
				auto sp2 = raw.find( ' ', sp1 + 1 );
				std::string dataType = (sp2 != std::string::npos)
				                     ? raw.substr( sp1 + 1, sp2 - sp1 - 1 )
				                     : raw.substr( sp1 + 1 );
				if      ( fieldClass == "Calculated" ) fieldType = "Calculation";
				else if ( fieldClass == "Summary"    ) fieldType = "Summary";
				else                                   fieldType = dataType;
			}
			else
			{
				fieldType = raw;
			}
		}

		// FieldRepetitions() returns the number of repetitions defined for the field
		fmx::DataUniquePtr repResult;
		fmx::TextUniquePtr repExpr;
		std::string repExprStr = "FieldRepetitions ( \"" + fileName + "\" ; \"" + fieldName + "\" )";
		repExpr->Assign( repExprStr.c_str(), fmx::Text::kEncoding_UTF8 );

		int repetitions = 1;
		if ( env->Evaluate( *repExpr, *repResult ) == 0 )
		{
			std::string raw = FMXTextToUTF8( repResult->GetAsText() );
			try { repetitions = std::stoi( raw ); } catch ( ... ) {}
		}

		if ( !first ) json += ',';
		first = false;
		json += "{\"name\":\"" + JsonEscape( fieldName ) + "\","
		      + "\"id\":" + fieldId + ","
		      + "\"type\":\"" + JsonEscape( fieldType )  + "\","
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
