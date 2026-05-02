import type { ReactNode } from "react";

interface ReviewDataTableProps {
  caption: string;
  emptyMessage: string;
  headers: string[];
  rowCount: number;
  children: ReactNode;
}

export function ReviewDataTable({
  caption,
  children,
  emptyMessage,
  headers,
  rowCount,
}: ReviewDataTableProps) {
  return (
    <div className="review-table-scroll">
      <table className="review-table">
        <caption>{caption}</caption>
        <thead>
          <tr>
            {headers.map((header) => (
              <th key={header} scope="col">
                {header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rowCount > 0 ? (
            children
          ) : (
            <tr>
              <td colSpan={headers.length}>{emptyMessage}</td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
