import Database from 'better-sqlite3';

// Initialize the database connection.
// Using verbose logging can be helpful for debugging complex queries.
const db = new Database('faucet.db', { verbose: console.log });

// Optimize database for performance and safety.
// WAL mode allows for better concurrency (multiple readers, one writer).
db.pragma('journal_mode = WAL');
// Normal sync is a good balance between speed and data safety.
db.pragma('synchronous = NORMAL');

// Create the requests table if it doesn't already exist.
// This schema tracks requests by IP and address to enforce cooldowns.
db.exec(`
  CREATE TABLE IF NOT EXISTS requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ip TEXT NOT NULL,
    address TEXT NOT NULL,
    timestamp INTEGER NOT NULL
  );
`);

// Create indexes to speed up queries on ip and address columns.
db.exec('CREATE INDEX IF NOT EXISTS idx_ip ON requests(ip);');
db.exec('CREATE INDEX IF NOT EXISTS idx_address ON requests(address);');

// Prepare SQL statements once for better performance.
const findLastRequestStmt = db.prepare(`
  SELECT timestamp FROM requests
  WHERE ip = ? OR address = ?
  ORDER BY timestamp DESC
  LIMIT 1
`);

const insertRequestStmt = db.prepare(`
  INSERT INTO requests (ip, address, timestamp)
  VALUES (?, ?, ?)
`);

/**
 * Checks if a user is allowed to request tokens based on their IP and address.
 * @param ip The user's IP address.
 * @param address The user's wallet address.
 * @param cooldownMs The required cooldown period in milliseconds.
 * @returns {boolean} True if the user can make a request, false otherwise.
 */
export function canRequest(ip: string, address: string, cooldownMs: number): boolean {
  const row = findLastRequestStmt.get(ip, address) as { timestamp: number } | undefined;
  
  // If no previous request is found, they can request.
  if (!row) {
    return true;
  }

  // Check if the time since the last request is greater than the cooldown period.
  return Date.now() - row.timestamp >= cooldownMs;
}

/**
 * Records a new faucet request in the database.
 * @param ip The user's IP address.
 * @param address The user's wallet address.
 */
export function recordRequest(ip: string, address: string): void {
  try {
    insertRequestStmt.run(ip, address, Date.now());
  } catch (error) {
    console.error('Failed to record request in DB:', error);
    // Depending on the desired behavior, you might want to re-throw the error
    // to let the calling function know the operation failed.
  }
}
