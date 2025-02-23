import { useEffect, useState } from 'react';

interface StoryInfo {
  title: string;
  url?: string;
  author: string;
  score: number;
  comments: number;
  domain?: string;
}

interface HourlyData {
  hour: string;
  stories: StoryInfo[];
  avg_score: number;
  total_comments: number;
  top_authors: [string, number][];  // [author, count]
  domains: [string, number][];      // [domain, count]
}

function App() {
  const [hourlyData, setHourlyData] = useState<HourlyData | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchLatestHour = async () => {
    try {
      setLoading(true);
      setError(null);
      // const response = await fetch('http://localhost:3000/api/latest');
      const response = await fetch('/api/latest');

      if (!response.ok) {
        throw new Error('Failed to fetch data');
      }
      const data = await response.json();
      setHourlyData(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred');
    } finally {
      setLoading(false);
    }
  };

  const triggerUpdate = async () => {
    try {
      setLoading(true);
      setError(null);
      // const response = await fetch('http://localhost:3000/api/update', {
      const response = await fetch('/api/update', {

        method: 'POST',
      });
      if (!response.ok) {
        throw new Error('Failed to trigger update');
      }
      await fetchLatestHour();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchLatestHour();
  }, []);

  if (loading) {
    return <div>Loading...</div>;
  }

  if (error) {
    return (
      <div>
        <div>Error: {error}</div>
        <button onClick={fetchLatestHour}>Retry</button>
      </div>
    );
  }

  if (!hourlyData) {
    return <div>No data available</div>;
  }

  return (
    <div className="container mx-auto p-4">

      <div className="flex justify-between items-center mb-6">
        <h1 className="text-2xl font-bold">
          Hacker News Summary for {new Date(hourlyData.hour).toLocaleString()}
        </h1>
        <button 
          onClick={triggerUpdate}
          className="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
        >
          Update Now
        </button>
      </div>
      <div className="bg-white p-4 rounded shadow">
          <h2 className="text-xl font-bold mb-4">Latest Stories</h2>
          <ul>
            {hourlyData.stories.map((story) => (
              <li key={story.title} className="mb-4">
                <a 
                  href={story.url} 
                  target="_blank" 
                  rel="noopener noreferrer"
                  className="text-blue-600 hover:text-blue-800 font-medium"
                >
                  {story.title}
                </a>
                <div className="text-sm text-gray-600">
                  by {story.author} | {story.score} points | {story.comments} comments
                  {story.domain && ` | ${story.domain}`}
                </div>
              </li>
            ))}
          </ul>
        </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <div className="bg-white p-4 rounded shadow">
          <h2 className="text-xl font-bold mb-4">Statistics</h2>
          <div>Average Score: {hourlyData.avg_score.toFixed(1)}</div>
          <div>Total Comments: {hourlyData.total_comments}</div>
        </div>

        <div className="bg-white p-4 rounded shadow">
          <h2 className="text-xl font-bold mb-4">Top Domains</h2>
          <ul>
            {hourlyData.domains.map(([domain, count]) => (
              <li key={domain} className="mb-2">
                {domain}: {count} stories
              </li>
            ))}
          </ul>
        </div>

        <div className="bg-white p-4 rounded shadow">
          <h2 className="text-xl font-bold mb-4">Top Authors</h2>
          <ul>
            {hourlyData.top_authors.map(([author, count]) => (
              <li key={author} className="mb-2">
                {author}: {count} stories
              </li>
            ))}
          </ul>
        </div>


      </div>
    </div>
  );
}

export default App; 