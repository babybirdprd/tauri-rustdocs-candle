'use client'; // Required for client-side interactivity

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';

// Assuming ProjectData and QueryDocResultItem might be defined elsewhere
// or we define simplified versions here for UI state.
interface Project {
  path: string;
  // other fields if needed by the query page, e.g., name
}

interface QueryResultItem {
  project_path: string;
  item_full_path: string;
  item_type: string;
  description_snippet?: string;
  score: number;
}

export default function QueryPage() {
  const [query, setQuery] = useState<string>('');
  const [selectedProjectPath, setSelectedProjectPath] = useState<string>(''); // Store the path
  const [availableProjects, setAvailableProjects] = useState<Project[]>([]); // To populate dropdown
  const [results, setResults] = useState<QueryResultItem[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // TODO: Load available projects for the dropdown.
  // This might come from the same source as the projects page,
  // or a dedicated Tauri command like `get_processed_project_paths`.
  useEffect(() => {
    async function fetchProjects() {
       // Using the main isLoading state for simplicity, could have a dedicated one
       setIsLoading(true);
       setError(null); // Clear previous query errors
      try {
         console.log("useEffect: Fetching processed project list...");
         const projectPaths = await invoke<string[]>('get_processed_project_list');
         console.log("Fetched project paths:", projectPaths);
         setAvailableProjects(projectPaths.map(path => ({ path })));
         if (projectPaths.length === 0) {
            // Optional: set a specific message if no projects are processed yet
            // setError("No projects have been processed yet. Please process a project on the 'Projects' page.");
         }
       } catch (err: any) {
        console.error("Failed to fetch projects:", err);
         setError("Failed to load project list from backend. Ensure backend is running and projects have been processed if expected.");
         setAvailableProjects([]); // Clear projects on error
       } finally {
         setIsLoading(false);
      }
    }
    fetchProjects();
   }, []); // Empty dependency array means this runs once on component mount

  const handleQuery = async () => {
    if (!query.trim()) {
      setError("Query cannot be empty.");
      return;
    }
    setIsLoading(true);
    setError(null);
    setResults([]); // Clear previous results
    try {
      console.log(`Invoking 'invoke_query_documentation' with query: "${query}", projectPath: "${selectedProjectPath || 'all'}"`);
      const queryResults = await invoke<QueryResultItem[]>('invoke_query_documentation', {
        naturalLanguageQuery: query.trim(), // Ensure key matches Rust struct
        projectPath: selectedProjectPath || null,
        numResults: 10 // Example: make this configurable later if needed
      });
      console.log("Query results from backend:", queryResults);
      setResults(queryResults);
      if (queryResults.length === 0) {
        setError("No results found for your query.");
      }
    } catch (err: any) {
      console.error("Failed to execute query:", err);
      setError(typeof err === 'string' ? err : err.message || "An unknown error occurred during query execution.");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="container mx-auto p-4">
      <h1 className="text-2xl font-bold mb-6 text-gray-800">Query Documentation</h1>

      <div className="space-y-4 p-6 border border-gray-200 rounded-lg shadow-sm bg-white">
        {/* Query Input */}
        <div>
          <label htmlFor="queryInput" className="block text-sm font-medium text-gray-700 mb-1">
            Enter your query:
          </label>
          <textarea
            id="queryInput"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="e.g., How does async work in Rust?"
            rows={4}
            className="w-full p-2.5 border border-gray-300 rounded-md focus:ring-blue-500 focus:border-blue-500 shadow-sm transition-shadow duration-150"
          />
        </div>

        {/* Project Context Dropdown (Optional) */}
        {availableProjects.length > 0 && ( // Only show if there are projects to select
          <div>
            <label htmlFor="projectSelect" className="block text-sm font-medium text-gray-700 mb-1">
              Filter by project (optional):
            </label>
            <select
              id="projectSelect"
              value={selectedProjectPath}
              onChange={(e) => setSelectedProjectPath(e.target.value)}
              className="w-full p-2.5 border border-gray-300 rounded-md bg-white focus:ring-blue-500 focus:border-blue-500 shadow-sm transition-shadow duration-150"
            >
              <option value="">All Processed Projects</option>
              {availableProjects.map(proj => (
                <option key={proj.path} value={proj.path}>{proj.path}</option>
              ))}
            </select>
          </div>
        )}

        {/* Submit Button */}
        <button
          onClick={handleQuery}
          disabled={isLoading || !query.trim()}
          className="w-full px-4 py-2.5 bg-blue-600 text-white font-semibold rounded-md hover:bg-blue-700 disabled:bg-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-opacity-50 transition ease-in-out duration-150"
        >
          {isLoading ? 'Searching...' : 'Search Documentation'}
        </button>
      </div>

      {/* Error Display */}
      {error && !isLoading && ( // Only show error if not loading
        <div className="mt-4 p-3 bg-red-100 text-red-700 border border-red-300 rounded-md shadow-sm">
          <strong>Error:</strong> {error}
        </div>
      )}

      {/* Results Area */}
      <div className="mt-8">
        <h2 className="text-xl font-semibold mb-4 text-gray-700">Results</h2>
        {isLoading && (
            <div className="text-center py-5">
                <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
                <p className="text-gray-500 mt-2">Loading results...</p>
            </div>
        )}
        {!isLoading && results.length === 0 && !error && (
          <div className="text-center text-gray-500 py-5">
            <p>No results to display.</p>
            <p>Enter a query above and click "Search Documentation".</p>
          </div>
        )}
        {!isLoading && results.length > 0 && (
          <div className="space-y-4">
            {results.map((item, index) => (
              <div key={index} className="p-4 border border-gray-200 rounded-lg shadow-sm bg-white hover:shadow-md transition-shadow duration-150">
                <h3 className="text-lg font-semibold text-blue-700 hover:underline cursor-pointer" title={item.item_full_path}>
                  {item.item_full_path}
                </h3>
                <p className="text-xs text-gray-500 mb-1">
                  Project: <span className="font-medium">{item.project_path}</span> | Type: <span className="font-medium">{item.item_type}</span>
                </p>
                <p className="text-sm text-gray-700 mb-2 leading-relaxed">
                  {item.description_snippet || 'No description available.'}
                </p>
                <p className="text-xs text-gray-600 font-medium">
                  Similarity Score: <span className="text-blue-600">{item.score.toFixed(4)}</span>
                </p>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
