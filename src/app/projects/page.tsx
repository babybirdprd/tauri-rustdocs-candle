'use client'; // Required for client-side interactivity in Next.js App Router

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri'; // For calling Rust backend

interface Project {
  path: string;
  status: 'idle' | 'processing' | 'processed' | 'error';
  message?: string; // For error messages or other info
}

export default function ProjectsPage() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [newProjectPath, setNewProjectPath] = useState<string>('');
  const [isLoading, setIsLoading] = useState<boolean>(false); // For loading state of an action

  // Function to load projects from backend (if stored) - Placeholder for now
  // useEffect(() => {
  //   async function loadProjects() {
  //     // const loadedProjects = await invoke<Project[]>('get_managed_projects');
  //     // setProjects(loadedProjects);
  //   }
  //   loadProjects();
  // }, []);

  const handleAddProject = () => {
    if (newProjectPath.trim() && !projects.find(p => p.path === newProjectPath.trim())) {
      setProjects([...projects, { path: newProjectPath.trim(), status: 'idle' }]);
      setNewProjectPath('');
    } else if (projects.find(p => p.path === newProjectPath.trim())) {
      // Optionally, notify user that project already exists
      console.warn("Project already added:", newProjectPath.trim());
      alert("Project path already added.");
    }
  };

  const handleRemoveProject = (pathToRemove: string) => {
    setProjects(projects.filter(p => p.path !== pathToRemove));
    // TODO: Optionally, call backend to remove/unmanage project
  };

  const handleProcessProject = async (projectPath: string) => {
    setProjects(prev => prev.map(p => p.path === projectPath ? { ...p, status: 'processing', message: '' } : p));
    setIsLoading(true);
    try {
      // This will call the Tauri command, which in turn calls the MCP tool.
      // The MCP tool `process_rust_project` is what we'll eventually connect this to.
      // For now, the Tauri command `invoke_process_rust_project` is a placeholder.
      const result = await invoke<string>('invoke_process_rust_project', { path: projectPath });
      setProjects(prev => prev.map(p => p.path === projectPath ? { ...p, status: 'processed', message: result } : p));
    } catch (error: any) {
      console.error("Failed to process project:", error);
      setProjects(prev => prev.map(p => p.path === projectPath ? { ...p, status: 'error', message: error.toString() } : p));
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="container mx-auto p-4">
      <h1 className="text-2xl font-bold mb-4 text-gray-800">Project Management</h1>

      {/* Add Project Form */}
      <div className="mb-6 p-4 border rounded-lg shadow-sm bg-white">
        <h2 className="text-xl font-semibold mb-3 text-gray-700">Add New Project</h2>
        <div className="flex space-x-2">
          <input
            type="text"
            value={newProjectPath}
            onChange={(e) => setNewProjectPath(e.target.value)}
            placeholder="Enter absolute path to Rust project"
            className="flex-grow p-2 border border-gray-300 rounded-md focus:ring-blue-500 focus:border-blue-500 shadow-sm"
          />
          <button
            onClick={handleAddProject}
            disabled={!newProjectPath.trim()}
            className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:bg-gray-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-opacity-50 transition ease-in-out duration-150"
          >
            Add Project
          </button>
        </div>
      </div>

      {/* Project List */}
      <div className="space-y-4">
        {projects.length === 0 && (
          <div className="text-center text-gray-500 py-5">
            <p>No projects added yet.</p>
            <p>Add a project path above to get started.</p>
          </div>
        )}
        {projects.map((project) => (
          <div key={project.path} className="p-4 border border-gray-200 rounded-lg shadow-sm bg-white hover:shadow-md transition-shadow duration-150">
            <div className="flex justify-between items-center">
              <div className="flex-grow mr-4 overflow-hidden">
                <h3 className="text-lg font-medium text-gray-800 truncate" title={project.path}>{project.path}</h3>
                <p className={`text-sm font-medium ${
                  project.status === 'processed' ? 'text-green-600' :
                  project.status === 'processing' ? 'text-yellow-600 animate-pulse' :
                  project.status === 'error' ? 'text-red-600' : 'text-gray-500'
                }`}>
                  Status: {project.status}
                  {project.message && <span className="ml-2 text-xs text-gray-600">({project.message})</span>}
                </p>
              </div>
              <div className="flex-shrink-0 space-x-2">
                <button
                  onClick={() => handleProcessProject(project.path)}
                  disabled={project.status === 'processing' || isLoading}
                  className="px-3 py-1.5 bg-green-500 text-white rounded-md hover:bg-green-600 disabled:bg-gray-400 text-sm focus:outline-none focus:ring-2 focus:ring-green-500 focus:ring-opacity-50 transition ease-in-out duration-150"
                >
                  {project.status === 'processing' ? 'Processing...' : 'Process'}
                </button>
                <button
                  onClick={() => handleRemoveProject(project.path)}
                  className="px-3 py-1.5 bg-red-500 text-white rounded-md hover:bg-red-600 text-sm focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-opacity-50 transition ease-in-out duration-150"
                >
                  Remove
                </button>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
